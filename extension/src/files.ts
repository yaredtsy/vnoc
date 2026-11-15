import * as path from "path";
import * as vscode from "vscode";

import type { TreeNode } from "@vnoc/shared/messages";

// Where a node is on disk and in its file: what a click, Go to Code and the Explorer's menu need. (They lived with the native tree.)

/** The file or folder of a path below the opened folder. */
export function absolute(relative: string): vscode.Uri {
  const folder = vscode.workspace.workspaceFolders?.[0];
  return vscode.Uri.file(folder ? path.join(folder.uri.fsPath, relative) : relative);
}

/** Where to put the cursor to show a node's definition: its first character. */
export function selectionOf(node: TreeNode): vscode.TextDocumentShowOptions {
  const r = node.range;
  return r ? { selection: new vscode.Range(r.start.line, r.start.character, r.start.line, r.start.character) } : {};
}
