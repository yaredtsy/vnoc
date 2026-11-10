import * as path from "path";
import * as vscode from "vscode";

import type { Range, TreeNode } from "@vnoc/shared/messages";
import { chainAt, findNode, nodeAtPath } from "@vnoc/shared/tree";
import { addDoc } from "./addDoc";
import type { Core } from "./core";
import type { Panels } from "./panels";
import type { SessionStore } from "./session";
import { absolute } from "./structureView";

// Every `vnoc.*` command: what a sidebar row, a card's right-click menu, the Explorer and the editor's menu run. Each one turns its argument into a
// node (`nodeFor`), then changes the session or shows something. The commands never keep state of their own.

const isTreeNode = (arg: unknown): arg is TreeNode => typeof arg === "object" && arg !== null && "kind" in arg && "children" in arg;

/** Cursor moves come in bursts; look at where it rests. */
const CURSOR_DEBOUNCE_MS = 200;

/** The editor whose cursor counts: the active one, else a visible code editor (clicking into a webview leaves none active). */
const cursorEditor = () => vscode.window.activeTextEditor ?? vscode.window.visibleTextEditors[0];

/** Runs a command and shows what went wrong as a message: menu clicks have nowhere else to report. */
const command = (run: (arg: unknown, selected?: unknown) => Promise<unknown> | unknown) => async (arg: unknown, selected?: unknown) => {
  try {
    await run(arg, selected);
  } catch (e) {
    void vscode.window.showErrorMessage(`V-NOC: ${(e as Error).message ?? e}`);
  }
};

export class Commands {
  constructor(
    private readonly core: Core,
    private readonly store: SessionStore,
    private readonly panels: Panels,
  ) {}

  /** Registers every command, and the cursor follower. */
  register(): vscode.Disposable[] {
    const on = (name: string, run: (arg: unknown, selected?: unknown) => Promise<unknown> | unknown) => vscode.commands.registerCommand(name, command(run));
    return [
      // with a node (the editor's or the Explorer's menu, a row) the canvas opens at it; from the palette it just opens
      on("vnoc.openCanvas", async (arg, selected) => (arg === undefined ? void this.panels.showCanvas() : this.openCanvasAt((await this.nodeFor(arg, selected)).id))),
      on("vnoc.focus", async (arg, selected) => this.focusInSidebar((await this.nodeFor(arg, selected)).id)),
      on("vnoc.showOnCanvas", async (arg, selected) => this.showOnCanvas((await this.nodeFor(arg, selected)).id)),
      on("vnoc.focusBack", () => this.store.dispatch({ type: "focusBack" })),
      on("vnoc.focusClear", () => this.store.dispatch({ type: "focusJump", depth: 0 })),
      on("vnoc.goToCode", async (arg, selected) => this.goToCode(await this.nodeFor(arg, selected))),
      on("vnoc.showDocs", (arg, selected) => this.showDocsBeside(arg, selected)),
      on("vnoc.addDoc", async (arg, selected) => {
        const node = await this.nodeFor(arg, selected);
        if (node.id === "/") throw new Error("Docs go on a folder inside the opened folder, not on the opened folder itself.");
        await this.addDocTo(node.id);
      }),
      on("vnoc.refreshStructure", () => this.core.notify("vnoc/rebuild")), // the way out if the tree ever does not follow the code
      this.followCursor(),
    ];
  }

  // ── which node ────────────────────────────────────────────────────────────────────────────────────────────────────────

  /** The nodes around the cursor: the file, then the class or function it is in (innermost last). */
  async chainAtCursor(): Promise<TreeNode[]> {
    const editor = cursorEditor();
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!editor || !folder) return [];
    const tree = this.core.tree ?? (await this.core.loadStructure()).root;
    const relative = path.relative(folder.uri.fsPath, editor.document.uri.fsPath).split(path.sep).join("/");
    return chainAt(tree, relative, editor.selection.active.line);
  }

  /**
   * The node a command is about. It starts from one of: a sidebar item (a tree node), a card's right-click menu (`{ nodeKey }`), the Explorer
   * (a Uri, with the list of selected Uris as the next argument), or the editor's menu (a Uri, nothing after it: the node at the cursor).
   */
  async nodeFor(arg: unknown, selected?: unknown): Promise<TreeNode> {
    if (isTreeNode(arg)) return arg;
    const tree = this.core.tree ?? (await this.core.loadStructure()).root;
    if (typeof arg === "object" && arg !== null && "nodeKey" in arg) {
      const hit = findNode(tree, String((arg as { nodeKey: unknown }).nodeKey));
      if (!hit) throw new Error("That node is not in the structure any more.");
      return hit;
    }
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (arg instanceof vscode.Uri && Array.isArray(selected) && folder) {
      const hit = nodeAtPath(tree, path.relative(folder.uri.fsPath, arg.fsPath).split(path.sep).join("/"));
      if (!hit) throw new Error("V-NOC has not read that file or folder (it is not Python, or it is ignored).");
      return hit;
    }
    const chain = await this.chainAtCursor();
    if (chain.length === 0) throw new Error("Put the cursor in a Python file that V-NOC has read.");
    return chain[chain.length - 1];
  }

  // ── what they do ──────────────────────────────────────────────────────────────────────────────────────────────────────

  /** Focus: the sidebar and the canvas both start at this node (the focus is shared), and the V-NOC side bar comes up. The canvas is not opened. */
  private async focusInSidebar(id: string) {
    this.store.dispatch({ type: "focus", id });
    await vscode.commands.executeCommand("workbench.view.extension.vnoc");
  }

  /** Show on Canvas: zoom the canvas into this node (and the sidebar follows), opening the canvas if it is closed. */
  private showOnCanvas(id: string) {
    this.store.dispatch({ type: "focus", id });
    this.panels.showCanvas();
  }

  /** Open Canvas, on a node: bring the canvas up with the node revealed and selected, keeping the focus unless it hides the node. The canvas finds the request in the session, whether it was open or not. */
  private openCanvasAt(id: string) {
    this.store.dispatch({ type: "reveal", id, canvas: true });
    this.panels.showCanvas();
  }

  /** Go to code: the definition in the editor (a folder is shown in the Explorer). */
  async goToCode(node: TreeNode) {
    const uri = absolute(node.path);
    if (node.kind === "folder") return vscode.commands.executeCommand("revealInExplorer", uri);
    const start = node.range ? new vscode.Position(node.range.start.line, node.range.start.character) : undefined;
    await vscode.window.showTextDocument(uri, start ? { selection: new vscode.Range(start, start) } : {});
  }

  /** `goToCode` for a node key (a card's button). */
  async goToCodeOf(key: string) {
    const tree = this.core.tree ?? (await this.core.loadStructure()).root;
    const node = findNode(tree, key);
    if (!node) throw new Error("That node is not in the structure any more.");
    await this.goToCode(node);
  }

  /** Opens (or reuses) the docs tab beside the editor. From a card or a row it shows that node; from the editor's menu it follows the cursor. */
  private async showDocsBeside(arg: unknown, selected: unknown) {
    const explicit = isTreeNode(arg) || (typeof arg === "object" && arg !== null && "nodeKey" in arg) || Array.isArray(selected);
    this.panels.showDocs(explicit ? (await this.nodeFor(arg, selected)).id : undefined);
    this.syncCursor(); // an unpinned tab shows the cursor node: make sure the session has it, also when no cursor has moved since the window opened
  }

  /** The "Add doc" flow for the node with this id (a sidebar item, or a card on the canvas). Resolves to whether a doc was added. */
  async addDocTo(key: string): Promise<boolean> {
    const structure = this.core.tree ? undefined : await this.core.loadStructure();
    const root = this.core.gitRoot ?? structure!.git_root;
    const node = findNode(this.core.tree!, key);
    if (!node || node.id === "/") throw new Error("That node is not in the structure any more.");
    return addDoc((method, params) => this.core.request(method, params), root, node, node.docs);
  }

  /** The text of a definition, from VS Code's document (so unsaved edits show). The path comes from the webview, so it must stay inside the opened folder. */
  async codeOf(relative: string, r: Range): Promise<string> {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) throw new Error("Open a folder first.");
    const file = path.resolve(folder.uri.fsPath, relative);
    if (!file.startsWith(folder.uri.fsPath + path.sep)) throw new Error(`${relative} is outside the opened folder`);
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
    return doc.getText(new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character));
  }

  // ── the cursor ────────────────────────────────────────────────────────────────────────────────────────────────────────

  /** Puts the node the text cursor is in into the session's `cursor`. */
  syncCursor() {
    void this.chainAtCursor().then((chain) => this.store.dispatch({ type: "cursor", id: chain[chain.length - 1]?.id ?? null }), () => {});
  }

  /** The session's `cursor` follows the cursor. Only a code editor moves it: clicking into a webview does not clear it. */
  private followCursor(): vscode.Disposable {
    let timer: NodeJS.Timeout | undefined;
    const soon = (editor: vscode.TextEditor | undefined) => {
      if (!editor) return;
      clearTimeout(timer);
      timer = setTimeout(() => this.syncCursor(), CURSOR_DEBOUNCE_MS);
    };
    return vscode.Disposable.from(
      vscode.window.onDidChangeTextEditorSelection((e) => soon(e.textEditor)),
      vscode.window.onDidChangeActiveTextEditor(soon),
      { dispose: () => clearTimeout(timer) },
    );
  }
}
