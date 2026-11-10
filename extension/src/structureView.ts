import * as path from "path";
import * as vscode from "vscode";

import type { TreeNode } from "@vnoc/shared/messages";

import { nodeUri } from "./cursorMark";

// The sidebar: the structure down to classes and functions. Calls are not here; they live on the canvas.
// The core already sorts like the explorer (folders first, then names; defs in source order), so this file only draws what it is given.

const DEF_ICON: Record<"class" | "function", string> = { class: "symbol-class", function: "symbol-function" };

/** What the tree draws: the focus bar (only while there is a focus), and the nodes. One provider gives both, so the bar sits inside the same tree. */
export type Row = { row: "focusBar" } | { row: "node"; node: TreeNode };

/** How the sidebar is doing: shown by `treeView.message` and `viewsWelcome` (package.json). */
export type SidebarState = { kind: "loading" | "ready" | "empty" } | { kind: "error"; message: string };

const FOCUS_BAR: Row = { row: "focusBar" };

export class StructureView implements vscode.TreeDataProvider<Row> {
  private readonly changed = new vscode.EventEmitter<Row | void>();
  readonly onDidChangeTreeData = this.changed.event;
  private readonly stateChanged = new vscode.EventEmitter<SidebarState>();
  readonly onDidChangeState = this.stateChanged.event;
  state: SidebarState = { kind: "loading" };

  private root: TreeNode | undefined;
  /** each node's parent (none for the nodes at the top, whose parent is the hidden root): `TreeView.reveal` needs it */
  private parents = new Map<string, TreeNode>();
  /** the focus stack: the tree then starts at its last node, and the focus bar names the way there */
  private focus: readonly string[] = [];
  /** One row object per node object, so `reveal` finds the row the tree was given (a newer structure has new node objects, and so new rows). */
  private readonly rows = new WeakMap<TreeNode, Row>();
  /** the node the text cursor is in: file and folder rows say so in their description (class and function rows get a decoration, cursorMark.ts) */
  private cursor: string | null = null;

  /** `load` asks the core; it is called again on every refresh. */
  constructor(private readonly load: () => Promise<TreeNode>) {}

  async refresh(): Promise<void> {
    if (!this.root) this.setState({ kind: "loading" });
    try {
      this.root = await this.load();
      this.parents = parentsOf(this.root);
      this.setState({ kind: this.root.children.length > 0 ? "ready" : "empty" });
    } catch (e) {
      this.root = undefined;
      this.setState({ kind: "error", message: String((e as Error).message ?? e) });
    }
    this.changed.fire();
  }

  private setState(state: SidebarState) {
    this.state = state;
    this.stateChanged.fire(state);
  }

  /** The focus stack to draw (empty: the whole tree). */
  setFocus(stack: readonly string[]): void {
    this.focus = stack;
    this.changed.fire();
  }

  /** The cursor moved: the two rows that are or were the cursor node are drawn again. */
  setCursor(id: string | null): void {
    const before = this.cursor;
    this.cursor = id;
    for (const changed of [before, id]) {
      const row = changed ? this.rowForId(changed) : undefined;
      if (row) this.changed.fire(row);
    }
  }

  /** The node this view holds for an id. (`reveal` wants the very object the view gave out, not an equal one from a newer structure.) */
  find(id: string): TreeNode | undefined {
    const search = (node: TreeNode): TreeNode | undefined => (node.id === id ? node : node.children.map(search).find((hit) => hit));
    return this.root?.children.map(search).find((hit) => hit);
  }

  /** The row of a node. */
  rowFor(node: TreeNode): Row {
    let row = this.rows.get(node);
    if (!row) this.rows.set(node, (row = { row: "node", node }));
    return row;
  }

  /** The row of the node with this id, if the tree has it. */
  rowForId(id: string): Row | undefined {
    const node = this.find(id);
    return node ? this.rowFor(node) : undefined;
  }

  private get focused(): TreeNode | undefined {
    const top = this.focus[this.focus.length - 1];
    return top ? this.find(top) : undefined;
  }

  getChildren(row?: Row): Row[] {
    if (row) return row.row === "node" ? row.node.children.map((c) => this.rowFor(c)) : [];
    const focused = this.focused;
    return focused ? [FOCUS_BAR, this.rowFor(focused)] : (this.root?.children ?? []).map((c) => this.rowFor(c));
  }

  getParent(row: Row): Row | undefined {
    if (row.row === "focusBar" || row.node.id === this.focus[this.focus.length - 1]) return undefined;
    const parent = this.parents.get(row.node.id);
    return parent ? this.rowFor(parent) : undefined;
  }

  getTreeItem(row: Row): vscode.TreeItem {
    if (row.row === "focusBar") return this.focusBarItem();
    const node = row.node;
    // the focused node is the top of the tree, and open (otherwise there would be nothing to see)
    const open = node.id === this.focus[this.focus.length - 1] ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.Collapsed;
    const state = node.children.length > 0 ? open : vscode.TreeItemCollapsibleState.None;
    const item = new vscode.TreeItem(node.name, state);
    item.id = node.id;
    // the flags the menus need, as words (`when` clauses test them with =~ /\bfunction\b/)
    item.contextValue = ["node", node.kind, node.docs > 0 ? "docs" : "", node.range ? "hasCode" : ""].filter(Boolean).join(" ");
    if (node.kind === "folder" || node.kind === "file") {
      // a resourceUri makes VS Code draw the file icon of the user's icon theme, as in the explorer
      item.resourceUri = absolute(node.path);
      item.iconPath = node.kind === "folder" ? vscode.ThemeIcon.Folder : vscode.ThemeIcon.File;
    } else {
      item.iconPath = new vscode.ThemeIcon(DEF_ICON[node.kind]);
      item.resourceUri = nodeUri(node.id); // so the cursor mark can decorate it; the icon stays the codicon (`iconPath` wins)
    }
    const description = [node.syntax_error ? "syntax error" : node.docs > 0 ? `📄 ${node.docs}` : "", this.cursor === node.id && (node.kind === "file" || node.kind === "folder") ? "●" : ""].filter(Boolean).join(" · ");
    if (description) item.description = description;
    if (node.link) item.tooltip = node.link === "marker" ? "Docs attached: the ID is in the code, so they follow it anywhere" : "Docs attached: found by path and name only, so they are lost if it moves";
    item.accessibilityInformation = { label: `${node.kind} ${node.name}${node.docs > 0 ? `, ${node.docs} docs` : ""}` };
    // a click (or Enter) runs `vnoc.open`: select, and show the node on the canvas if it is open, else open the code (commands.ts)
    item.command = { command: "vnoc.open", title: "Open", arguments: [node] };
    return item;
  }

  /** "Focus: app › checkout": the last two names, with the way there in the tooltip. Back and Clear are its inline buttons. */
  private focusBarItem(): vscode.TreeItem {
    const names = this.focus.map((id) => this.find(id)?.name ?? "?");
    const shown = names.length > 2 ? `… › ${names.slice(-2).join(" › ")}` : names.join(" › ");
    const item = new vscode.TreeItem(`Focus: ${shown}`, vscode.TreeItemCollapsibleState.None);
    item.iconPath = new vscode.ThemeIcon("target");
    item.contextValue = "focusBar";
    item.tooltip = `Focus: ${names.join(" › ")}`;
    return item;
  }
}

export function absolute(relative: string): vscode.Uri {
  const folder = vscode.workspace.workspaceFolders?.[0];
  return vscode.Uri.file(folder ? path.join(folder.uri.fsPath, relative) : relative);
}

export function selectionOf(node: TreeNode): vscode.TextDocumentShowOptions {
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
