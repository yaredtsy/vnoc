import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { DocInfo, MissingNode, MovePlan, TrashItem } from "@vnoc/shared/messages";

// Remove, Restore and Empty Trash (docs/11-missing-relink-remove.md §5). The core plans and refuses; VS Code does the move, so a doc of that node
// open in a tab follows it. `moveToTrash` and `restoreEntry` have no questions in them: the commands wrap them with the conversation, and the test
// suite calls them as they are.

type Request = <T>(method: string, params?: object) => Promise<T>;

const nameOf = (m: MissingNode) => m.qualname || m.path;

/** The node folder moves as a workspace edit. A refusal (or a failure) is an error: the core has written only an empty entry, which the next listing removes. */
async function move(plan: MovePlan): Promise<void> {
  const edit = new vscode.WorkspaceEdit();
  edit.renameFile(vscode.Uri.file(plan.from), vscode.Uri.file(plan.to));
  if (!(await vscode.workspace.applyEdit(edit))) throw new Error("VS Code did not move the docs. Nothing was removed.");
}

/** Remove, without the question: the core's plan (it refuses an ID that is not Missing), then the move. */
export async function moveToTrash(request: Request, id: string): Promise<void> {
  await move(await request<MovePlan>("vnoc/trash", { id }));
}

/** Restore, without the question: the core works out where the node goes back to (and refuses when docs are there), then the move. */
export async function restoreEntry(request: Request, entry: string): Promise<void> {
  await move(await request<MovePlan>("vnoc/restorePlan", { entry }));
}

/** Remove… on a Missing row. Resolves to whether the docs were removed. */
export async function removeMissing(request: Request, missing: MissingNode): Promise<boolean> {
  const docs = await request<DocInfo[]>("vnoc/docs", { id: missing.id });
  const answer = await vscode.window.showWarningMessage(
    `Remove the docs of ${nameOf(missing)}?`,
    {
      modal: true,
      detail: `${docs.length === 0 ? "It has no doc files." : docs.map((d) => `• ${d.file}`).join("\n")}\n\nThe docs move to V-NOC's trash on this computer. The deletion shows in git; commit it to remove them for everyone. "Restore Removed Doc…" brings them back.`,
    },
    "Remove",
  );
  if (answer !== "Remove") return false;
  await moveToTrash(request, missing.id);
  return true;
}

/** Restore Removed Doc…: pick one removed node. Resolves to whether it was restored. */
export async function restoreRemoved(request: Request): Promise<boolean> {
  const entries = await request<TrashItem[]>("vnoc/trashList");
  if (entries.length === 0) {
    void vscode.window.showInformationMessage("V-NOC: nothing has been removed.");
    return false;
  }
  const picked = await vscode.window.showQuickPick(
    entries.map((e) => ({ label: e.name, description: `${e.docs} ${e.docs === 1 ? "doc" : "docs"} · removed ${e.removed_at.replace("T", " ").replace("Z", " UTC")}`, entry: e })),
    { title: "Restore Removed Doc", placeHolder: "Which one?", matchOnDescription: true },
  );
  if (!picked) return false;
  await restoreEntry(request, picked.entry.entry);
  void vscode.window.showInformationMessage(`V-NOC: the docs of ${picked.entry.name} are back. If the code is gone, they show under Missing.`);
  return true;
}

/** Empty Trash: the only permanent delete in V-NOC, and it goes to the OS trash where there is one. */
export async function emptyTrash(request: Request, gitRoot: string): Promise<boolean> {
  await request("vnoc/trashList"); // the listing removes the empty entries, so they are not counted
  const folder = path.join(gitRoot, ".vnoc", "local", "trash");
  const count = fs.existsSync(folder) ? fs.readdirSync(folder, { withFileTypes: true }).filter((e) => e.isDirectory()).length : 0;
  if (count === 0) {
    void vscode.window.showInformationMessage("V-NOC: the trash is empty.");
    return false;
  }
  const answer = await vscode.window.showWarningMessage(
    `Empty V-NOC's trash? ${count} removed ${count === 1 ? "node" : "nodes"}, ${size(folderBytes(folder))}.`,
    { modal: true, detail: "They go to the system trash if there is one. After that they cannot be restored from V-NOC." },
    "Empty Trash",
  );
  if (answer !== "Empty Trash") return false;
  await vscode.workspace.fs.delete(vscode.Uri.file(folder), { recursive: true, useTrash: true });
  return true;
}

function folderBytes(dir: string): number {
  let total = 0;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const file = path.join(dir, entry.name);
    total += entry.isDirectory() ? folderBytes(file) : entry.isFile() ? fs.statSync(file).size : 0;
  }
  return total;
}

function size(bytes: number): string {
  return bytes < 1024 ? `${bytes} B` : bytes < 1024 * 1024 ? `${(bytes / 1024).toFixed(1)} KB` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
