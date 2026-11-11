import * as vscode from "vscode";

import type { TreeNode } from "@vnoc/shared/messages";

import { documentOf } from "./relink";

// Write ID (docs/11-missing-relink-remove.md §4): a node found by name gets its ID back in the code. The core decides and refuses with a reason.

type Request = <T>(method: string, params?: object) => Promise<T>;

interface WriteIdResult {
  written: boolean;
  notice: string | null;
}

/** Resolves to whether the ID was written. */
export async function writeId(request: Request, gitRoot: string, node: TreeNode): Promise<boolean> {
  const { document, param, wasDirty } = await documentOf(gitRoot, node);
  const result = await request<WriteIdResult>("vnoc/writeId", { key: node.id, document: param });
  // the ID is in the editor's buffer now: save it so the core's index sees it, unless the user had unsaved work of their own
  if (result.written && !wasDirty) await document?.save();
  if (result.notice) void vscode.window.showWarningMessage(`V-NOC: ${result.notice}`);
  if (result.written) void vscode.window.showInformationMessage(`V-NOC: the ID of ${node.name} is in the code again.`);
  return result.written;
}
