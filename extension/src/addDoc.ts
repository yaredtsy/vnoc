import * as path from "path";
import * as vscode from "vscode";

import type { TreeNode } from "./structureView";

// "Add doc" (07-ui/02-design.md): ask for a title, ask the once-per-project question, let the core attach, open the new doc.
// The core does the work (an ID line in the code, then `.vnoc/`); this file is only the conversation with the user.

type Request = <T>(method: string, params?: object) => Promise<T>;

interface ProjectInfo {
  initialized: boolean;
  markers: boolean;
}

interface AttachResult {
  id: string;
  doc: string;
  id_in_code: boolean;
  notice: string | null;
}

/** The line a new doc's cursor starts on: under the `# Title` heading (the frontmatter takes four lines). */
const FIRST_BODY_LINE = 5;

/** What the first-attach question says V-NOC will write, by kind of node. */
const MARKER: Record<TreeNode["kind"], string> = {
  function: "an ID line to {name}'s docstring",
  class: "an ID line to {name}'s docstring",
  file: "a `# FileID:` comment at the top of {name}",
  folder: "a .vnoc-id file to the folder {name}",
};

/**
 * `node` is a node of `vnoc/structure`: its `id` is `path::qualname` (or `#n`) for a function or class, the path from the git root for a
 * file or folder. `existingDocs` is how many docs the node has: the first one is pre-filled with "Overview", later ones start empty.
 * Resolves to whether a doc was added. Failures are shown to the user here; the promise only rejects for bugs.
 */
export async function addDoc(request: Request, gitRoot: string, node: TreeNode, existingDocs: number): Promise<boolean> {
  try {
    const key = node.id;
    const [file, qualname = ""] = key.split("::");
    const name = qualname.replace(/#\d+$/, "").split(".").pop() || file.split("/").pop() || file;

    const title = await vscode.window.showInputBox({ title: "Add doc", prompt: `Title of the new doc for ${name}`, value: existingDocs === 0 ? "Overview" : "", validateInput: (t) => (t.trim() ? undefined : "A doc needs a title") });
    if (title === undefined) return false; // Esc: nothing has been written

    // the one time V-NOC edits the user's code without being asked in so many words
    let markers: boolean | undefined;
    if (!(await request<ProjectInfo>("vnoc/projectInfo")).initialized) {
      const answer = await vscode.window.showInformationMessage(
        `V-NOC will add ${MARKER[node.kind].replace("{name}", name)} and create a .vnoc/ folder in this repository.`,
        { modal: true },
        "Add doc",
        "Add without ID",
      );
      if (!answer) return false;
      markers = answer === "Add doc";
    }

    // a folder has no document: the core writes its `.vnoc-id` itself
    const document = node.kind === "folder" ? undefined : await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(gitRoot, file)));
    const wasDirty = document?.isDirty ?? false;
    const result = await request<AttachResult>("vnoc/attach", {
      key,
      title: title.trim(),
      markers,
      document: document && { uri: document.uri.toString(), version: document.version, text: document.getText() },
    });

    // The ID is in the editor's buffer now. Save it so the core's index sees it, unless the user had unsaved work of their own.
    if (result.id_in_code && !wasDirty) await document?.save();
    if (result.notice) void vscode.window.showWarningMessage(`V-NOC: ${result.notice}`);

    const doc = await vscode.window.showTextDocument(vscode.Uri.file(result.doc));
    const start = new vscode.Position(Math.min(FIRST_BODY_LINE, doc.document.lineCount - 1), 0);
    doc.selection = new vscode.Selection(start, start);
    return true;
  } catch (e) {
    void vscode.window.showErrorMessage(`V-NOC: ${(e as Error).message ?? e}`);
    return false;
  }
}
