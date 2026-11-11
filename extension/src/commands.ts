import * as path from "path";
import * as vscode from "vscode";

import type { Range, TreeNode } from "@vnoc/shared/messages";
import { chainAt, findNode, nodeAtPath } from "@vnoc/shared/tree";
import type { Events } from "@vnoc/shared/messages";
import { MISSING_PIN } from "@vnoc/shared/tree";
import { addDoc } from "./addDoc";
import { type AttentionRow, goToProblem } from "./attentionView";
import type { Core } from "./core";
import type { Panels } from "./panels";
import type { SessionStore } from "./session";
import { attachMissingHere, relinkMissing } from "./relink";
import { writeId } from "./writeId";
import { searchNodes } from "./search";
import { emptyTrash, removeAllMissing, removeMissing, restoreRemoved } from "./trash";
import { type Row, absolute, selectionOf } from "./structureView";

// Every `vnoc.*` command: what a sidebar row, a card's right-click menu, the Explorer and the editor's menu run. Each one turns its argument into a
// node (`nodeFor`), then changes the session or shows something. The commands never keep state of their own.

const isTreeNode = (arg: unknown): arg is TreeNode => typeof arg === "object" && arg !== null && "kind" in arg && "children" in arg;
/** What the native tree gives a menu command: the row. */
const isNodeRow = (arg: unknown): arg is Row & { row: "node" } => typeof arg === "object" && arg !== null && "row" in arg && (arg as Row).row === "node";

const isAttentionRow = <R extends AttentionRow["row"]>(arg: unknown, row: R): arg is Extract<AttentionRow, { row: R }> => typeof arg === "object" && arg !== null && (arg as AttentionRow).row === row;

/** Cursor moves come in bursts; look at where it rests. */
const CURSOR_DEBOUNCE_MS = 200;

/** "Expand All Below" opens at most this many nodes: the open set is in the saved session and travels with every broadcast. */
const EXPAND_ALL_LIMIT = 500;

/** The editor whose cursor counts: the active one, else a visible code editor (clicking into a webview leaves none active). */
const cursorEditor = () => vscode.window.activeTextEditor ?? vscode.window.visibleTextEditors[0];

/** Runs a command and shows what went wrong as a message: menu clicks have nowhere else to report. */
const command = (run: (arg: unknown, selected?: unknown) => Promise<unknown> | unknown) => async (arg: unknown, selected?: unknown) => {
  try {
    return await run(arg, selected);
  } catch (e) {
    void vscode.window.showErrorMessage(`V-NOC: ${(e as Error).message ?? e}`);
  }
};

export class Commands {
  constructor(
    private readonly core: Core,
    private readonly store: SessionStore,
    private readonly panels: Panels,
    /** selects the node's row in the sidebar tree and brings the V-NOC side bar up */
    private readonly revealRow: (id: string) => Promise<void>,
  ) {}

  private readonly request = <T>(method: string, params?: object) => this.core.request<T>(method, params);

  /** The git root: node ids and the core's paths start there. */
  private async root(): Promise<string> {
    return this.core.gitRoot ?? (await this.core.loadStructure()).git_root;
  }

  /** Registers every command, and the cursor follower. */
  register(): vscode.Disposable[] {
    const on = (name: string, run: (arg: unknown, selected?: unknown) => Promise<unknown> | unknown) => vscode.commands.registerCommand(name, command(run));
    return [
      // with a node (the editor's or the Explorer's menu, a row) the canvas opens at it; from the palette it just opens
      on("vnoc.openCanvas", async (arg, selected) => (arg === undefined ? void this.panels.showCanvas() : this.openCanvasAt((await this.nodeFor(arg, selected)).id))),
      on("vnoc.open", (arg) => (isNodeRow(arg) ? this.open(arg.node) : isTreeNode(arg) ? this.open(arg) : undefined)),
      on("vnoc.focusJump", (depth) => this.store.dispatch({ type: "focusJump", depth: Number(depth) })),
      // the inline ✕ on a FOCUS-view row: drop this step and those after it
      on("vnoc.focusDrop", (step) => this.store.dispatch({ type: "focusJump", depth: Math.max(0, Number((step as { depth?: number }).depth) - 1) })),
      on("vnoc.showLog", () => this.core.showLog()),
      on("vnoc.revealInSidebar", async (arg, selected) => {
        const node = await this.nodeFor(arg, selected);
        this.store.dispatch({ type: "reveal", id: node.id, canvas: false }); // steps the focus back if it hides the node, selects it
        await this.revealRow(node.id);
      }),
      on("vnoc.revealInExplorer", async (arg, selected) => vscode.commands.executeCommand("revealInExplorer", absolute((await this.nodeFor(arg, selected)).path))),
      on("vnoc.openCanvasBeside", async (arg, selected) => (arg === undefined ? void this.panels.showCanvas({ beside: true }) : this.openCanvasAt((await this.nodeFor(arg, selected)).id, { beside: true }))),
      on("vnoc.expand", async (arg, selected) => this.store.dispatch({ type: "expand", ids: [(await this.nodeFor(arg, selected)).id] })),
      on("vnoc.collapse", async (arg, selected) => this.store.dispatch({ type: "toggle", id: (await this.nodeFor(arg, selected)).id })),
      on("vnoc.expandAll", async (arg, selected) => this.expandAll(await this.nodeFor(arg, selected))),
      on("vnoc.collapseAll", () => this.store.dispatch({ type: "collapseAll" })),
      on("vnoc.showCodeInCard", async (arg, selected) => this.viewCommand({ name: "showCode", key: (await this.nodeFor(arg, selected)).id })),
      on("vnoc.hideCodeInCard", async (arg, selected) => this.viewCommand({ name: "hideCode", key: (await this.nodeFor(arg, selected)).id })),
      on("vnoc.fitView", () => this.viewCommand({ name: "fitView" })),
      on("vnoc.copyPath", async (arg, selected) => {
        const node = await this.nodeFor(arg, selected);
        await vscode.env.clipboard.writeText(node.range ? `${node.path}:${node.range.start.line + 1}` : node.path);
      }),
      on("vnoc.clearSelection", () => this.store.dispatch({ type: "select", id: null })),
      on("vnoc.search", async () => searchNodes(this.core.tree ?? (await this.core.loadStructure()).root, this.store, (node) => this.open(node))),
      // Ctrl/Cmd+Enter in the sidebar: focus the selected node
      on("vnoc.focusSelection", () => {
        const s = this.store.get();
        const id = s.contexts[s.active].selected;
        if (id) this.store.dispatch({ type: "focus", id });
      }),
      on("vnoc.focus", async (arg, selected) => this.focusInSidebar((await this.nodeFor(arg, selected)).id)),
      on("vnoc.showOnCanvas", async (arg, selected) => this.showOnCanvas((await this.nodeFor(arg, selected)).id)),
      on("vnoc.focusBack", () => this.store.dispatch({ type: "focusBack" })),
      on("vnoc.focusClear", () => this.store.dispatch({ type: "focusJump", depth: 0 })),
      on("vnoc.goToCode", async (arg, selected) => this.goToCode(await this.nodeFor(arg, selected))),
      on("vnoc.showDocs", (arg, selected) => this.showDocsBeside(arg, selected)),
      // the NEEDS ATTENTION rows (a row is `{ row: "missing" | "problem", … }`; nothing here changes the session)
      on("vnoc.openMissingDocs", (arg) => (isAttentionRow(arg, "missing") ? this.panels.showDocs(MISSING_PIN + arg.node.id) : undefined)),
      on("vnoc.openProblem", (arg) => (isAttentionRow(arg, "problem") ? goToProblem(arg.problem) : undefined)),
      on("vnoc.relink", async (arg) => {
        if (!isAttentionRow(arg, "missing")) return;
        const tree = this.core.tree ?? (await this.core.loadStructure()).root;
        return relinkMissing(this.request, await this.root(), arg.node, tree);
      }),
      on("vnoc.removeMissing", async (arg) => (isAttentionRow(arg, "missing") ? removeMissing(this.core, arg.node) : undefined)),
      on("vnoc.restoreRemoved", () => restoreRemoved(this.core)),
      on("vnoc.removeAllMissing", () => removeAllMissing(this.core, this.core.missing)),
      on("vnoc.emptyTrash", async () => emptyTrash(this.core, await this.root(), this.core.missing.length)),
      on("vnoc.attachMissingDocHere", async (arg, selected) => {
        const node = await this.nodeFor(arg, selected);
        if (node.id === "/") throw new Error("Docs go on a folder inside the opened folder, not on the opened folder itself.");
        return attachMissingHere(this.request, await this.root(), this.core.missing, node);
      }),
      on("vnoc.writeId", async (arg, selected) => {
        const node = await this.nodeFor(arg, selected);
        if (node.node_id === null) throw new Error(`${node.name} has no docs, so there is no ID to write.`);
        if (node.link === "marker") throw new Error(`The ID of ${node.name} is already in the code.`);
        return writeId(this.request, await this.root(), node);
      }),
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
    if (isNodeRow(arg)) return arg.node;
    const tree = this.core.tree ?? (await this.core.loadStructure()).root;
    if (typeof arg === "object" && arg !== null && "nodeKey" in arg) {
      const hit = findNode(tree, String((arg as { nodeKey: unknown }).nodeKey));
      if (!hit) throw new Error("That node is not in the structure any more.");
      return hit;
    }
    const folder = vscode.workspace.workspaceFolders?.[0];
    // The Explorer passes the clicked Uri and the list of selected ones. The editor menu and a tab's menu pass the Uri alone: the active editor's own
    // document means "the node at the cursor"; any other Uri (a right-click on a background tab) is that file.
    const explorer = Array.isArray(selected);
    if (arg instanceof vscode.Uri && folder && (explorer || vscode.window.activeTextEditor?.document.uri.toString() !== arg.toString())) {
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
  private openCanvasAt(id: string, opts: { beside?: boolean } = {}) {
    this.store.dispatch({ type: "reveal", id, canvas: true });
    this.panels.showCanvas(opts);
  }

  /** Opens every closed node below this one on the canvas, up to a limit. */
  private async expandAll(node: TreeNode) {
    const ids: string[] = [];
    const walk = (n: TreeNode) => {
      if (n.children.length === 0 || ids.length >= EXPAND_ALL_LIMIT) return;
      ids.push(n.id);
      n.children.forEach(walk);
    };
    walk(node);
    this.store.dispatch({ type: "expand", ids });
    if (ids.length >= EXPAND_ALL_LIMIT) void vscode.window.showInformationMessage(`V-NOC opened the first ${EXPAND_ALL_LIMIT} nodes below ${node.name}. Open the rest where you need them.`);
  }

  /** A menu entry that changes only what the canvas draws, not the session. */
  private viewCommand(data: Events["viewCommand"]) {
    this.panels.emit("viewCommand", data);
  }

  /**
   * A click on a sidebar row (docs/10-ui-rework/02-sidebar.md §4): the canvas is where you are working while it is open, so a click shows the node there;
   * the code comes along beside it, never on top of it. With the canvas closed it opens the code as a preview tab, like a click in the Explorer.
   */
  private async open(node: TreeNode) {
    this.store.dispatch({ type: "select", id: node.id });
    const mode = vscode.workspace.getConfiguration("vnoc").get<"auto" | "code" | "canvas">("sidebar.click", "auto");
    if (mode === "canvas" || (mode === "auto" && this.panels.canvas)) {
      this.store.dispatch({ type: "reveal", id: node.id, canvas: true });
      const canvas = this.panels.showCanvas({ preserveFocus: true });
      // a code editor already visible in another column follows the click too (canvas and code side by side); none is opened in the canvas's column
      const code = vscode.window.visibleTextEditors.find((e) => e.viewColumn !== undefined && e.viewColumn !== canvas.viewColumn);
      if (code && node.kind !== "folder") await this.showCode(node, { viewColumn: code.viewColumn, preview: true, preserveFocus: true });
      return;
    }
    if (node.kind !== "folder") await this.showCode(node, { preview: true, preserveFocus: true });
  }

  /** The definition in an editor tab (the file, for a file). */
  private async showCode(node: TreeNode, options: vscode.TextDocumentShowOptions) {
    await vscode.window.showTextDocument(absolute(node.path), { ...selectionOf(node), ...options });
  }

  /**
   * Go to code: the definition in the editor, always (a folder is shown in the Explorer). With the canvas in front it opens in the column beside it,
   * so the canvas is not covered, and the keyboard goes to the code.
   */
  async goToCode(node: TreeNode) {
    if (node.kind === "folder") return vscode.commands.executeCommand("revealInExplorer", absolute(node.path));
    await this.showCode(node, this.panels.canvas?.active ? { viewColumn: vscode.ViewColumn.Beside } : {});
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
    const otherTab = arg instanceof vscode.Uri && vscode.window.activeTextEditor?.document.uri.toString() !== arg.toString();
    const explicit = isTreeNode(arg) || isNodeRow(arg) || (typeof arg === "object" && arg !== null && "nodeKey" in arg) || Array.isArray(selected) || otherTab;
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
