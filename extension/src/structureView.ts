import * as path from "path";
import * as vscode from "vscode";

// The sidebar: the structure down to classes and functions. Calls are not here; they live on the canvas.
// The core already sorts like the explorer (folders first, then names; defs in source order), so this file only draws what it is given.

export type Kind = "folder" | "file" | "class" | "function";

/** The shape the core sends for `vnoc/structure` (crates/vnoc/src/index/tree.rs). */
export interface TreeNode {
  id: string;
  kind: Kind;
  name: string;
  /** relative to the opened folder (not the git root); empty for the root */
  path: string;
  range: { start: { line: number; character: number }; end: { line: number; character: number } } | null;
  syntax_error: boolean;
  children: TreeNode[];
}

const DEF_ICON: Record<"class" | "function", string> = { class: "symbol-class", function: "symbol-function" };

export class StructureView implements vscode.TreeDataProvider<TreeNode> {
  private readonly changed = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.changed.event;
  private root: TreeNode | undefined;

  /** `load` asks the core; it is called again on every refresh. */
  constructor(private readonly load: () => Promise<TreeNode>) {}

  /** `announce`: show a failure as a message (the Refresh button); at startup it only leaves the view empty. */
  async refresh(announce = false): Promise<void> {
    try {
      this.root = await this.load();
    } catch (e) {
      this.root = undefined;
      if (announce) vscode.window.showErrorMessage(`V-NOC: ${(e as Error).message ?? e}`);
    }
    this.changed.fire();
  }

  getChildren(node?: TreeNode): TreeNode[] {
    return node ? node.children : (this.root?.children ?? []);
  }

  getTreeItem(node: TreeNode): vscode.TreeItem {
    const state = node.children.length > 0 ? vscode.TreeItemCollapsibleState.Collapsed : vscode.TreeItemCollapsibleState.None;
    const item = new vscode.TreeItem(node.name, state);
    item.id = node.id;
    item.contextValue = node.kind; // for `when` clauses on context menus
    if (node.kind === "folder" || node.kind === "file") {
      // a resourceUri makes VS Code draw the file icon of the user's icon theme, as in the explorer
      item.resourceUri = absolute(node.path);
      item.iconPath = node.kind === "folder" ? vscode.ThemeIcon.Folder : vscode.ThemeIcon.File;
    } else {
      item.iconPath = new vscode.ThemeIcon(DEF_ICON[node.kind]);
    }
    if (node.syntax_error) item.description = "syntax error";
    // folders just expand; the rest open the file (at the definition, for classes and functions)
    if (node.kind !== "folder") item.command = { command: "vscode.open", title: "Open", arguments: [absolute(node.path), selectionOf(node)] };
    return item;
  }
}

function absolute(relative: string): vscode.Uri {
  const folder = vscode.workspace.workspaceFolders?.[0];
  return vscode.Uri.file(folder ? path.join(folder.uri.fsPath, relative) : relative);
}

function selectionOf(node: TreeNode): vscode.TextDocumentShowOptions {
  const r = node.range;
  return r ? { selection: new vscode.Range(r.start.line, r.start.character, r.start.line, r.start.character) } : {};
}
