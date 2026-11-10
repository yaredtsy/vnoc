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
  /** how many docs are attached, and how the node was found again (crates/vnoc/src/index/tree.rs) */
  docs: number;
  link: "marker" | "name" | null;
  children: TreeNode[];
}

const DEF_ICON: Record<"class" | "function", string> = { class: "symbol-class", function: "symbol-function" };

export class StructureView implements vscode.TreeDataProvider<TreeNode> {
  private readonly changed = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.changed.event;
  private root: TreeNode | undefined;
  /** each node's parent (none for the nodes at the top, whose parent is the hidden root): `TreeView.reveal` needs it */
  private parents = new Map<string, TreeNode>();
  /** the node the focus stack is on: the tree then shows only that node and what is under it */
  private focusId: string | undefined;

  /** `load` asks the core; it is called again on every refresh. */
  constructor(private readonly load: () => Promise<TreeNode>) {}

  /** `announce`: show a failure as a message (the Refresh button); at startup it only leaves the view empty. */
  async refresh(announce = false): Promise<void> {
    try {
      this.root = await this.load();
      this.parents = parentsOf(this.root);
    } catch (e) {
      this.root = undefined;
      if (announce) vscode.window.showErrorMessage(`V-NOC: ${(e as Error).message ?? e}`);
    }
    this.changed.fire();
  }

  /** Focus on this node (or none: the whole tree). */
  setFocus(id: string | undefined): void {
    this.focusId = id;
    this.changed.fire();
  }

  getParent(node: TreeNode): TreeNode | undefined {
    return node.id === this.focusId ? undefined : this.parents.get(node.id);
  }

  /** The node this view holds for an id. (`reveal` wants the very object the view gave out, not an equal one from a newer structure.) */
  find(id: string): TreeNode | undefined {
    const search = (node: TreeNode): TreeNode | undefined => (node.id === id ? node : node.children.map(search).find((hit) => hit));
    return this.root?.children.map(search).find((hit) => hit);
  }

  getChildren(node?: TreeNode): TreeNode[] {
    if (node) return node.children;
    const focused = this.focusId ? this.find(this.focusId) : undefined;
    return focused ? [focused] : (this.root?.children ?? []);
  }

  getTreeItem(node: TreeNode): vscode.TreeItem {
    // the focused node is the top of the tree, and open (otherwise there would be nothing to see)
    const open = node.id === this.focusId ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.Collapsed;
    const state = node.children.length > 0 ? open : vscode.TreeItemCollapsibleState.None;
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
    else if (node.docs > 0) item.description = `📄 ${node.docs}`;
    if (node.link) item.tooltip = node.link === "marker" ? "Docs attached: the ID is in the code, so they follow it anywhere" : "Docs attached: found by path and name only, so they are lost if it moves";
    // folders just expand; the rest open the file (at the definition, for classes and functions)
    if (node.kind !== "folder") item.command = { command: "vscode.open", title: "Open", arguments: [absolute(node.path), selectionOf(node)] };
    return item;
  }
}

export function absolute(relative: string): vscode.Uri {
  const folder = vscode.workspace.workspaceFolders?.[0];
  return vscode.Uri.file(folder ? path.join(folder.uri.fsPath, relative) : relative);
}

function selectionOf(node: TreeNode): vscode.TextDocumentShowOptions {
  const r = node.range;
  return r ? { selection: new vscode.Range(r.start.line, r.start.character, r.start.line, r.start.character) } : {};
}

function parentsOf(root: TreeNode): Map<string, TreeNode> {
  const parents = new Map<string, TreeNode>();
  const walk = (parent: TreeNode) => {
    for (const child of parent.children) {
      if (parent !== root) parents.set(child.id, parent);
      walk(child);
    }
  };
  walk(root);
  return parents;
}
