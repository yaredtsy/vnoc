import * as path from "path";
import * as vscode from "vscode";

import type { MissingNode, TreeNode } from "@vnoc/shared/messages";
import { allNodes, repoPathOfId } from "@vnoc/shared/tree";

// Relink (docs/11-missing-relink-remove.md §3): a Missing node's docs go to another place. Two ways in, both ending in `vnoc/relink`:
//   · on a Missing row: pick the new place (every node of the same kind that has no docs yet)
//   · on the new code (a row, a card, the editor's menu): pick the Missing doc (those of that kind, and those that do not say)
// The core decides and refuses with a reason; this file only asks the user and shows the answer.

type Request = <T>(method: string, params?: object) => Promise<T>;

interface RelinkResult {
  written: "id" | "anchorOnly";
  notice: string | null;
}

const ICON: Record<TreeNode["kind"], string> = { folder: "folder", file: "file", class: "symbol-class", function: "symbol-function" };

/** A Missing node of this kind, or one that does not say its kind (an older `node.toml`). */
export const fitsKind = (missing: MissingNode, kind: TreeNode["kind"]) => missing.kind === null || missing.kind === kind;

/** Whether a node of the tree can take a Missing node's docs: the same kind (any, when it does not say), and no docs of its own. */
export const canTake = (missing: MissingNode, node: TreeNode, root: TreeNode) => node.id !== root.id && node.node_id === null && fitsKind(missing, node.kind);

const nameOf = (m: MissingNode) => m.qualname || m.path;

/** Relink… on a Missing row: asks for the new place. Resolves to whether the docs moved. */
export async function relinkMissing(request: Request, gitRoot: string, missing: MissingNode, root: TreeNode): Promise<boolean> {
  const items = allNodes(root)
    .filter((n) => canTake(missing, n, root))
    .map((node) => ({ label: node.name, description: node.range ? `${node.path}:${node.range.start.line + 1}` : node.path, iconPath: new vscode.ThemeIcon(ICON[node.kind]), node }));
  if (items.length === 0) {
    void vscode.window.showInformationMessage(`V-NOC: there is no ${missing.kind ?? "node"} without docs to move ${nameOf(missing)} to.`);
    return false;
  }
  const picked = await vscode.window.showQuickPick(items, { title: `Relink ${nameOf(missing)}`, placeHolder: "Where is it now? Find it by name or path", matchOnDescription: true });
  return picked ? relinkTo(request, gitRoot, missing, picked.node) : false;
}

/** Attach a Missing Doc Here…: asks which Missing doc belongs to this node. Resolves to whether the docs moved. */
export async function attachMissingHere(request: Request, gitRoot: string, missing: MissingNode[], node: TreeNode): Promise<boolean> {
  if (node.node_id !== null) throw new Error(`${node.name} already has its own docs.`);
  const fits = missing.filter((m) => fitsKind(m, node.kind));
  if (fits.length === 0) {
    void vscode.window.showInformationMessage(`V-NOC: no Missing doc is for a ${node.kind}.`);
    return false;
  }
  const items = fits.map((m) => ({ label: nameOf(m), description: `${m.qualname ? `${m.path} · ` : ""}${m.docs} ${m.docs === 1 ? "doc" : "docs"}`, iconPath: new vscode.ThemeIcon(m.kind ? ICON[m.kind as TreeNode["kind"]] ?? "question" : "question"), missing: m }));
  const picked = await vscode.window.showQuickPick(items, { title: `Attach a Missing doc to ${node.name}`, placeHolder: "Which doc belongs here?", matchOnDescription: true });
  return picked ? relinkTo(request, gitRoot, picked.missing, node) : false;
}

/** The core call, with the open document the ID is written into (as `addDoc` sends it). */
async function relinkTo(request: Request, gitRoot: string, missing: MissingNode, node: TreeNode): Promise<boolean> {
  // a folder has no document: the core writes its `.vnoc-id` itself. The file is named by the node's `id`, which starts at the git root
  // (`node.path` is relative to the opened folder, which can be a subfolder of the repo).
  const file = repoPathOfId(node.id);
  const document = node.kind === "folder" ? undefined : await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(gitRoot, file)));
  const wasDirty = document?.isDirty ?? false;
  const result = await request<RelinkResult>("vnoc/relink", { id: missing.id, to: node.id, document: document && { uri: document.uri.toString(), version: document.version, text: document.getText() } });
  // the ID is in the editor's buffer now: save it so the core's index sees it, unless the user had unsaved work of their own
  if (result.written === "id" && !wasDirty) await document?.save();
  if (result.notice) void vscode.window.showWarningMessage(`V-NOC: ${result.notice}`);
  void vscode.window.showInformationMessage(`V-NOC: the docs of ${nameOf(missing)} are now on ${node.name}.`);
  return true;
}
