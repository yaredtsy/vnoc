import * as path from "path";
import * as vscode from "vscode";

import { type CallId, isCallId, listedSites, resolveCall, siteChoice } from "@vnoc/shared/calls.ts";
import type { CallTreeNode, Range, TreeNode } from "@vnoc/shared/messages";
import { chainAt, findNode, nodeAtPath } from "@vnoc/shared/tree";
import type { CodeResult, EditDiffParams, EditSaveParams, EditSaveResult, EditStartResult, Events } from "@vnoc/shared/messages";
import { reindent } from "@vnoc/shared/indent";
import { MISSING_PIN } from "@vnoc/shared/tree";
import { addDoc } from "./addDoc";
import type { DraftDocs } from "./draftDocs";
import { dropsId } from "@vnoc/shared/idLine";
import { SAVE_ANYWAY, afterLocate, answerForAlready, docLocate, firstStep, idQuestion, outcomeAfterApply, wholeRange, withDraft } from "./saveDraft";
import type { Docs } from "./docs";
import type { CallTrees } from "./callTree";
import { type AttentionRow, goToProblem } from "./attentionView";
import type { Core } from "./core";
import type { Surfaces } from "./surfaces";
import { type ContextId, type ContextState, MAIN } from "@vnoc/shared/session.ts";
import type { SessionStore } from "./session";
import { attachMissingHere, relinkMissing } from "./relink";
import type { CliSetup } from "./cliSetup";
import { writeId } from "./writeId";
import { emptyTrash, removeAllMissing, removeMissing, restoreRemoved } from "./trash";
import { absolute, selectionOf } from "./files";
import { pickItem } from "./tours";
import type { ToursAnswer } from "@vnoc/shared/tours.ts";

// Every `vnoc.*` command: what a sidebar row, a card's right-click menu, the Explorer and the editor's menu run. Each one turns its argument into a
// node (`nodeFor`), then changes the session or shows something. The commands never keep state of their own.

const isTreeNode = (arg: unknown): arg is TreeNode => typeof arg === "object" && arg !== null && "kind" in arg && "children" in arg;
const isAttentionRow = <R extends AttentionRow["row"]>(arg: unknown, row: R): arg is Extract<AttentionRow, { row: R }> => typeof arg === "object" && arg !== null && (arg as AttentionRow).row === row;

/** The editor's or a tab's menu passes the file's Uri alone; the Explorer passes it with the list of selected ones. From an editor the canvas opens beside it. */
const isEditorArg = (arg: unknown, selected: unknown): boolean => arg instanceof vscode.Uri && !Array.isArray(selected);

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

/** Where the function of a draft is in a document's text now, or null: the core's `vnoc/locate`. A parameter of Save so that a test can pass its own. */
export type Locate = (key: string, document: { uri: string; version: number; text: string }) => Promise<Range | null>;

/** Asks whether a draft that takes its function's `ID:` away may be saved (§7): the draft's key; true is Save Anyway. A parameter of Save so that a test can answer. */
export type AskToDropId = (key: string) => Promise<boolean>;

export class Commands {
  constructor(
    private readonly core: Core,
    private readonly store: SessionStore,
    private readonly panels: Surfaces,
    /** brings the V-NOC side bar up (`vnoc.structure.focus`); the page scrolls the selected row into view by itself */
    private readonly showSidebar: () => Promise<void>,
    private readonly callTrees: CallTrees,
    /** the in-memory documents "Show Diff" puts beside the file */
    private readonly draftDocs: DraftDocs,
    /** a doc's path is checked here (§8): inside `<git root>/.vnoc/nodes/`, a `.md` */
    private readonly docs: Docs,
    /** the command in PATH, the coding agents, and the offer made after a first attach (07 §3) */
    private readonly cli: CliSetup,
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
      // Open Canvas takes no node: it brings the canvas up as it was (the title button, the palette). With a node it is Open in Canvas
      on("vnoc.openCanvas", () => void this.panels.showCanvas()),
      // reveal the node on the canvas, opening it if closed; from an editor or a tab it opens BESIDE it, so the code stays in view (08 §4)
      on("vnoc.openInCanvas", async (arg, selected) => this.openCanvasAt((await this.nodeFor(arg, selected)).id, { beside: isEditorArg(arg, selected) })),
      // a click on a node, from the palette or a test: the sidebar row's click, in MAIN
      // (an ID lost row of NEEDS ATTENTION is a node of the tree: its click is the sidebar's)
      on("vnoc.open", (arg) => (isTreeNode(arg) ? this.rowClick(MAIN, arg.id) : isAttentionRow(arg, "lostId") ? this.rowClick(MAIN, arg.item.key) : undefined)),
      on("vnoc.focusJump", (depth) => this.store.dispatch({ type: "focusJump", depth: Number(depth) })),
      on("vnoc.showLog", () => this.core.showLog()),
      on("vnoc.revealInSidebar", async (arg, selected) => {
        const node = await this.nodeFor(arg, selected);
        this.store.dispatch({ type: "reveal", id: node.id, canvas: false }); // steps the focus back if it hides the node, selects it
        await this.showSidebar();
      }),
      on("vnoc.revealInExplorer", async (arg, selected) => vscode.commands.executeCommand("revealInExplorer", absolute((await this.nodeFor(arg, selected)).path))),
      // a card's own id: a structure key, or a call id for a call card (whose `nodeKey` is its callee)
      on("vnoc.expand", async (arg, selected) => this.store.dispatch({ type: "expand", ids: [await this.cardIdOf(arg, selected)] }, this.contextOf(arg))),
      on("vnoc.collapse", async (arg, selected) => this.store.dispatch({ type: "toggle", id: await this.cardIdOf(arg, selected) }, this.contextOf(arg))),
      // the call menu (08 §3): Explore follows the call into its own context; Go to Call Site goes to its line, or asks when it is made in several places
      on("vnoc.explore", (arg) => {
        const call = this.callFor(arg);
        this.store.dispatch({ type: "explore", call: call.id, activate: true }, call.context);
      }),
      on("vnoc.goToCallSite", (arg) => this.goToCallSite(this.callFor(arg).node)),
      on("vnoc.openTour", (arg) => this.openTour(arg)),
      // **Show Tours** in a node's menu (card and row), when the node is on tours: the same as a click on its `▶ N`
      on("vnoc.showTours", async (arg, selected) => this.showToursOf((await this.nodeFor(arg, selected)).id, this.contextOf(arg))),
      // the menu of a tour's card and of a stop's row (design 04 §7): the real file, and MAIN on that node (the tour stays where it was)
      on("vnoc.tourGoToCode", (arg) => this.goToCodeOf(this.tourNodeKey(arg))),
      on("vnoc.tourRevealInMain", (arg) => {
        this.store.dispatch({ type: "reveal", id: this.tourNodeKey(arg), canvas: true }, MAIN);
        this.store.dispatch({ type: "activate", id: MAIN }, MAIN);
      }),
      on("vnoc.closeContext", (arg) => this.closeContext(this.contextOf(arg))),
      on("vnoc.closeOtherContexts", (arg) => this.closeOtherContexts(this.contextOf(arg))),
      on("vnoc.expandAll", async (arg, selected) => this.expandAll(await this.nodeFor(arg, selected))),
      on("vnoc.collapseAll", () => this.store.dispatch({ type: "collapseAll" })),
      on("vnoc.showCodeInCard", async (arg, selected) => this.viewCommand({ name: "showCode", key: await this.cardIdOf(arg, selected) })),
      on("vnoc.hideCodeInCard", async (arg, selected) => this.viewCommand({ name: "hideCode", key: await this.cardIdOf(arg, selected) })),
      // the page shows the card's code if it is not shown, then asks `editStart`: the draft lives in the page (docs/17 E6)
      on("vnoc.editInCard", async (arg, selected) => this.viewCommand({ name: "edit", key: await this.cardIdOf(arg, selected) })),
      on("vnoc.fitView", () => this.viewCommand({ name: "fitView" })),
      on("vnoc.copyPath", async (arg, selected) => {
        const node = await this.nodeFor(arg, selected);
        await vscode.env.clipboard.writeText(node.range ? `${node.path}:${node.range.start.line + 1}` : node.path);
      }),
      on("vnoc.clearSelection", () => this.store.dispatch({ type: "select", id: null })),
      // *V-NOC: Search…* is the sidebar's box (05 §2's fallback: VS Code re-sorts a QuickPick's items by its own matcher, so the core's order could not be shown there): the
      // STRUCTURE view comes up and the cursor goes into its box. A string argument is the text to start with (the window suite uses it); a node or anything else is not.
      on("vnoc.search", async (arg) => {
        await this.showSidebar();
        await this.panels.whenSidebarDrew(5000); // an event sent to a page that has not started is lost
        this.panels.emit("viewCommand", { name: "focusSearch", text: typeof arg === "string" ? arg : undefined });
      }),
      on("vnoc.focus", async (arg, selected) => this.focusInSidebar((await this.nodeFor(arg, selected)).id)),
      on("vnoc.focusBack", () => this.store.dispatch({ type: "focusBack" })),
      on("vnoc.focusClear", () => this.store.dispatch({ type: "focusJump", depth: 0 })),
      on("vnoc.goToCode", async (arg, selected) => this.goToCode(await this.nodeFor(arg, selected))),
      on("vnoc.showDocs", (arg, selected) => this.showDocsBeside(arg, selected)),
      on("vnoc.showCalls", async (arg, selected) => this.showCalls(await this.nodeFor(arg, selected), isEditorArg(arg, selected))),
      // the NEEDS ATTENTION rows (a row is `{ row: "missing" | "problem", … }`; nothing here changes the session)
      on("vnoc.openMissingDocs", (arg) => (isAttentionRow(arg, "missing") ? this.panels.showDocs(MISSING_PIN + arg.node.id) : undefined)),
      on("vnoc.openNotRead", (arg) => (isAttentionRow(arg, "notRead") && arg.item.kind === "file" ? this.openAt(arg.item.path, { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } }) : undefined)),
      on("vnoc.openProblem", (arg) => (isAttentionRow(arg, "problem") ? goToProblem(arg.problem) : undefined)),
      on("vnoc.relink", async (arg) => {
        if (!isAttentionRow(arg, "missing")) return;
        const tree = this.core.tree ?? (await this.core.loadStructure()).root;
        return relinkMissing(this.request, await this.root(), arg.node, tree);
      }),
      on("vnoc.removeMissing", async (arg) => (isAttentionRow(arg, "missing") ? removeMissing(this.core, arg.node) : undefined)),
      on("vnoc.restoreRemoved", () => restoreRemoved(this.core)),
      on("vnoc.removeAllMissing", () => removeAllMissing(this.core, this.core.missing)),
      on("vnoc.emptyTrash", () => emptyTrash(this.core, this.core.missing.length)),
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
      // the command line (07 §3): the palette only. Each answers what it did, for the window suite; `setUpAgents` takes `{ agents, confirm: true }` to skip its two questions
      on("vnoc.installCli", () => this.cli.installCli()),
      on("vnoc.uninstallCli", () => this.cli.uninstallCli()),
      on("vnoc.setUpAgents", (arg) => this.cli.setUpAgents(arg)),
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
   * The node a command is about. It starts from one of: a node, an ID lost row of NEEDS ATTENTION, a card's or a sidebar row's right-click menu (`{ nodeKey }`, from `nodeContext`),
   * the Explorer (a Uri, with the list of selected Uris as the next argument), or the editor's menu (a Uri, nothing after it: the node at the cursor).
   */
  async nodeFor(arg: unknown, selected?: unknown): Promise<TreeNode> {
    if (isTreeNode(arg)) return arg;
    const tree = this.core.tree ?? (await this.core.loadStructure()).root;
    if (isAttentionRow(arg, "lostId")) {
      const hit = findNode(tree, arg.item.key);
      if (!hit) throw new Error("That node is not in the structure any more.");
      return hit;
    }
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
      if (!hit) throw new Error("V-NOC has not read that file or folder (it is not a file V-NOC reads, or it is ignored).");
      return hit;
    }
    const chain = await this.chainAtCursor();
    if (chain.length === 0) throw new Error("Put the cursor in a file V-NOC reads.");
    return chain[chain.length - 1];
  }

  /** The context a menu entry is about: the one the card or the row is in (`contextId`, from `nodeContext`), else the active one. */
  private contextOf(arg: unknown): ContextId {
    const id = typeof arg === "object" && arg !== null && "contextId" in arg ? (arg as { contextId: unknown }).contextId : undefined;
    return typeof id === "string" && id in this.store.get().contexts ? id : this.store.get().active;
  }

  /**
   * The call a call card's menu is about, from `callId` in what `nodeContext` wrote (08 §6): its id, the context it is in, and its node in the call trees
   * the host holds, which has its sites (undefined when the tree is not held: nothing is known of its sites).
   */
  callFor(arg: unknown): { id: CallId; context: ContextId; node: CallTreeNode | undefined } {
    const id = typeof arg === "object" && arg !== null && "callId" in arg ? (arg as { callId: unknown }).callId : undefined;
    if (typeof id !== "string" || !isCallId(id)) throw new Error("That is not a call.");
    return { id, context: this.contextOf(arg), node: resolveCall(this.callTrees.all(), id) };
  }

  /** The id of the card a menu entry is about: a call id for a call card, else its structure node's key. */
  private async cardIdOf(arg: unknown, selected?: unknown): Promise<string> {
    return typeof arg === "object" && arg !== null && (arg as { role?: unknown }).role === "call" ? this.callFor(arg).id : (await this.nodeFor(arg, selected)).id;
  }

  // ── what they do ──────────────────────────────────────────────────────────────────────────────────────────────────────

  /** Focus: the sidebar and the canvas both start at this node (the focus is shared), and the V-NOC side bar comes up. The canvas is not opened. */
  private async focusInSidebar(id: string) {
    this.store.dispatch({ type: "focus", id });
    await vscode.commands.executeCommand("workbench.view.extension.vnoc");
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
   * A click on a sidebar row (docs/16-ui-redesign/07-sidebar-rows.md §4): the canvas is where you are working while it is open, so a click shows the node
   * there, in the row's context; the code comes along beside it, never on top of it. With the canvas closed it opens the code as a preview tab, like a click
   * in the Explorer. The page has sent the `select` intent already; it is sent again here for a caller that has not (the palette, a test): it changes nothing twice.
   */
  async rowClick(context: ContextId, id: string): Promise<void> {
    const node = findNode(this.core.tree ?? (await this.core.loadStructure()).root, id);
    if (!node) throw new Error("That node is not in the structure any more.");
    this.store.dispatch({ type: "select", id: node.id }, context);
    const mode = vscode.workspace.getConfiguration("vnoc").get<"auto" | "code" | "canvas">("sidebar.click", "auto");
    if (mode === "canvas" || (mode === "auto" && this.panels.canvas)) {
      this.store.dispatch({ type: "reveal", id: node.id, canvas: true }, context);
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

  /**
   * Show Calls (docs/16-ui-redesign/05-calls-as-nodes.md §7): Open in Canvas on the function (beside the editor when run from one), then open it. The host
   * sees it is open and fetches its calls. The answer decides what to say: a failure, or a class that calls nothing when it is created.
   */
  async showCalls(node: TreeNode, fromEditor = false): Promise<void> {
    if (node.kind !== "function" && node.kind !== "class") throw new Error("Calls start at a function or a class.");
    // R7: a class's own calls are under its `__init__`. A class with one of its own shows those; one without (it may inherit one) is its own entry
    const init = node.kind === "class" ? node.children.find((c) => c.name === "__init__") : undefined;
    if (init) return this.showCalls(init, fromEditor);
    this.openCanvasAt(node.id, { beside: fromEditor });
    this.store.dispatch({ type: "expand", ids: [node.id] });
    void this.callTrees.settled(node.id).then((answer) => {
      if (answer.state === "failed") return void vscode.window.showErrorMessage(`V-NOC could not read what ${node.name} calls: ${answer.message}`);
      if (answer.result.root.children.length > 0) return;
      void vscode.window.showInformationMessage(node.kind === "class" ? `${node.name} calls nothing when it is created; open a method to see its calls.` : `${node.name} calls nothing.`);
    });
  }

  /**
   * Go to Call Site when a call has several sites (04 §2.1): a list with the line, the file and the line's own text; **moving through it previews each
   * site** in the editor beside the canvas (`preserveFocus`), as Go to References does; Enter goes there for good, Escape leaves the editor as it was.
   */
  async pickCallSite(sites: { path: string; range: Range }[]) {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) throw new Error("Open a folder first.");
    const lineText = async (file: string, line: number) => {
      try {
        return (await vscode.workspace.openTextDocument(vscode.Uri.file(path.resolve(folder.uri.fsPath, file)))).lineAt(line).text;
      } catch {
        return "";
      }
    };
    const items = await Promise.all(sites.map(async (site) => ({ site, ...siteChoice({ ...site.range, path: site.path }, null, await lineText(site.path, site.range.start.line)) })));
    const pick = vscode.window.createQuickPick<(typeof items)[number]>();
    pick.title = "Go to Call Site";
    pick.ignoreFocusOut = true;
    pick.matchOnDetail = true;
    pick.items = items;
    pick.onDidChangeActive(([item]) => item && void this.openAt(item.site.path, item.site.range, { preserveFocus: true }).catch(() => {}));
    pick.onDidAccept(() => {
      const [item] = pick.selectedItems;
      pick.hide();
      if (item) void this.openAt(item.site.path, item.site.range).catch(() => {});
    });
    pick.onDidHide(() => pick.dispose());
    pick.show();
  }

  /** Go to Call Site (08 §3): one site goes there; several are a list that previews each. A call whose sites name no file of the project has nowhere to go. */
  private async goToCallSite(node: CallTreeNode | undefined): Promise<void> {
    const sites = node ? listedSites(node).map((site) => ({ path: site.path, range: { start: site.start, end: site.end } })) : [];
    if (sites.length === 0) throw new Error("This call has no site to go to: its calls are not loaded yet, or none is in a file of the project.");
    if (sites.length === 1) return this.openAt(sites[0].path, sites[0].range);
    return this.pickCallSite(sites);
  }

  /** Close (08 §5): an EXPLORE context goes with its children; MAIN stays. */
  /** The QuickPick of **Open Tour…** (design/07 §8; `ignoreFocusOut`: from the bar's ⋯ the menu gives focus back to the page a moment after the picker opens, and a picker that closes with focus would be gone at once), the suite's way in: `pickAnswers` is the label to pick (`null`: VS Code's own), `picksShown` every list shown, as its lines (a line starting `— ` is a separator). */
  readonly tourPick: { pickAnswers: string[] | null; picksShown: string[][] } = { pickAnswers: null, picksShown: [] };

  /** The node selected in the window's active context (a tour's: the current stop's), if the tree has it. */
  private selectedKey(): string | null {
    const s = this.store.get();
    const key = s.contexts[s.active]?.selected ?? null;
    return key !== null && this.core.tree && findNode(this.core.tree, key) ? key : null;
  }

  /** **Ask for a Tour…** from a node's menu, the palette, a button: the node's key for `vnoc/tourAsk` (a menu names its node; the palette and the bar's buttons mean the selected one), or none. */
  async askKeyOf(arg: unknown, selected?: unknown): Promise<string | undefined> {
    const named = isTreeNode(arg) || (typeof arg === "object" && arg !== null && "nodeKey" in arg);
    if (named) return (await this.nodeFor(arg, selected)).id;
    return this.selectedKey() ?? undefined;
  }

  /**
   * **Open Tour…**. With a tour's id as its argument (`executeCommand("vnoc.openTour", id)`: the host's ⋯ and the suites) it opens that tour and asks nothing. Otherwise the core's list is a QuickPick in the
   * core's order: with a node selected, the tours on it first under a line *"on reserve_stock"* (`vnoc/tours { first_on }`; the page sorts nothing), then the others.
   */
  private async openTour(id?: unknown): Promise<void> {
    if (typeof id === "string" && id !== "") {
      const { tours } = await this.request<ToursAnswer>("vnoc/tours");
      const row = tours.find((r) => r.id === id);
      if (!row) throw new Error(`There is no tour with the id ${id}.`);
      await this.openTourById(row.id, row.title);
      return;
    }
    if (!this.core.tree) await this.core.loadStructure();
    const key = this.selectedKey();
    // a node the core refuses (the project's root is every tour's) is no node here: the plain list
    const { tours } = await (key !== null ? this.request<ToursAnswer>("vnoc/tours", { first_on: key }).catch(() => this.request<ToursAnswer>("vnoc/tours")) : this.request<ToursAnswer>("vnoc/tours"));
    if (tours.length === 0) {
      void vscode.window.showInformationMessage("There are no tours yet. Ask your coding agent for one: “give me a tour of …”.");
      return;
    }
    const on = tours.filter((r) => r.on !== undefined && r.on !== null);
    const rest = tours.filter((r) => !on.includes(r));
    const name = key !== null && this.core.tree ? (findNode(this.core.tree, key)?.name ?? key) : "";
    const separator = (label: string): vscode.QuickPickItem => ({ label, kind: vscode.QuickPickItemKind.Separator });
    const items: (vscode.QuickPickItem & { row?: ToursAnswer["tours"][number] })[] =
      on.length > 0 ? [separator(`on ${name}`), ...on.map(pickItem), ...(rest.length > 0 ? [separator("other tours")] : []), ...rest.map(pickItem)] : tours.map(pickItem);
    this.tourPick.picksShown.push(items.map((i) => (i.kind === vscode.QuickPickItemKind.Separator ? `— ${i.label}` : i.label)));
    const want = this.tourPick.pickAnswers?.shift();
    const picked = this.tourPick.pickAnswers !== null ? items.find((i) => i.row !== undefined && i.label === want) : await vscode.window.showQuickPick(items, { title: "Open Tour", placeHolder: "Pick a tour", matchOnDescription: true, matchOnDetail: true, ignoreFocusOut: true });
    if (!picked?.row) return;
    await this.openTourById(picked.row.id, picked.row.title);
  }

  /** Opens a tour as the window's tour context (one that was open is replaced) and brings the canvas up: Open Tour… and the notification's **Open Tour** do the same. */
  async openTourById(id: string, title: string | undefined): Promise<void> {
    // the session checks its intents against the structure, and the host reads it only while a page asks: with the canvas closed there is none yet
    if (!this.core.tree) await this.core.loadStructure();
    this.store.dispatch({ type: "tourOpen", id, title });
    await this.panels.showCanvas();
  }

  /**
   * `▶ N` and **Show Tours** (design 07 §6): the node is selected in its own context (which becomes the active one: the list follows the active context's selection), the TOURS
   * section is switched to **On the selected node** and shown. A tour context has no selection of this kind: the node is selected in MAIN instead.
   */
  async showToursOf(key: string, context: ContextId): Promise<void> {
    if (!this.core.tree) await this.core.loadStructure();
    const where = this.store.get().contexts[context]?.kind === "tour" || !this.store.get().contexts[context] ? MAIN : context;
    this.store.dispatch({ type: "activate", id: where }, where);
    this.store.dispatch({ type: "select", id: key }, where);
    this.store.dispatch({ type: "tourView", view: "onSelection" }, where);
    await this.showSidebar();
    await this.panels.whenSidebarDrew(5000); // an event sent to a page that has not started is lost
    this.panels.emit("viewCommand", { name: "showTours" });
  }

  /**
   * **Ask for a Tour…** from a node's menu or the palette, and a row's **Ask for a New Tour**: the TOURS section's prompt builder opens at the top of the section, filled in from where it
   * was opened (`key`: a node; `tour`: that tour's root, question and detail). The sentence is the core's, made in the form.
   */
  async askForTour(from: { key?: string; tour?: string }): Promise<void> {
    await this.showSidebar();
    await this.panels.whenSidebarDrew(5000); // an event sent to a page that has not started is lost
    this.panels.emit("viewCommand", { name: "askForTour", key: from.key, text: from.tour });
  }

  /** **Reveal Root in Main** on a tour's row: MAIN selects the tour's first stop and is the context the canvas draws. The tour stays where it was. A root with no node in the tree has nothing to reveal. */
  async revealTourRoot(key: string | null): Promise<void> {
    if (key === null) throw new Error("That tour's first stop is not in the structure any more.");
    if (!this.core.tree) await this.core.loadStructure();
    this.store.dispatch({ type: "reveal", id: key, canvas: true }, MAIN);
    this.store.dispatch({ type: "activate", id: MAIN }, MAIN);
    await this.panels.showCanvas();
  }

  /** The structure node a tour's menu acts on: `nodeKey` of what the card or the row said. A stop whose node is gone has none. */
  private tourNodeKey(arg: unknown): string {
    const key = (arg as { nodeKey?: unknown } | null)?.nodeKey;
    if (typeof key !== "string") throw new Error("That stop's code is not in the structure any more.");
    return key;
  }

  private closeContext(id: ContextId): void {
    if (this.store.get().contexts[id]?.kind === "explore") this.store.dispatch({ type: "closeContext", id }, id);
  }

  /** Close Others: every EXPLORE context but this one and the ones it hangs from (closing one of those would close this one with it). */
  private closeOtherContexts(keep: ContextId): void {
    const s = this.store.get();
    const keepers = new Set<ContextId>();
    for (let c: ContextState | undefined = s.contexts[keep]; c; c = c.parent === null ? undefined : s.contexts[c.parent]) keepers.add(c.id);
    for (const id of s.order) {
      if (keepers.has(id) || !this.store.get().contexts[id]) continue; // an earlier close may have taken it with it
      this.closeContext(id);
    }
  }

  /** A file below the opened folder, at a range (a call site). Beside the canvas when it is in front, as Go to Code does. */
  async openAt(relative: string, r: Range, opts: { preserveFocus?: boolean } = {}) {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) throw new Error("Open a folder first.");
    const file = path.resolve(folder.uri.fsPath, relative);
    if (!file.startsWith(folder.uri.fsPath + path.sep)) throw new Error(`${relative} is outside the opened folder`);
    const selection = new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character);
    await vscode.window.showTextDocument(vscode.Uri.file(file), { selection, preserveFocus: opts.preserveFocus, ...(this.panels.canvas?.active ? { viewColumn: vscode.ViewColumn.Beside } : {}) });
  }

  /**
   * A doc row of the search was picked (`Requests.showDoc`). The page revealed the node and sent the click already. A node in the tree: the docs tab pinned to it with the doc chosen.
   * A Missing node: the tab pinned to it the way *Needs attention* does. Neither: its code was not read, there is nothing to pin, and the `.md` opens in an editor tab.
   */
  async showDoc({ node, nodeId, missing, path }: { node: string | null; nodeId: string; missing: boolean; path: string }): Promise<void> {
    if (node !== null) this.panels.showDocs(node, path);
    else if (missing) this.panels.showDocs(MISSING_PIN + nodeId, path);
    else await this.docs.open(path);
  }

  /** Opens (or reuses) the docs tab beside the editor. From a card or a row it shows that node; from the editor's menu it follows the cursor. */
  private async showDocsBeside(arg: unknown, selected: unknown) {
    const otherTab = arg instanceof vscode.Uri && vscode.window.activeTextEditor?.document.uri.toString() !== arg.toString();
    const explicit = isTreeNode(arg) || (typeof arg === "object" && arg !== null && "nodeKey" in arg) || Array.isArray(selected) || otherTab;
    this.panels.showDocs(explicit ? (await this.nodeFor(arg, selected)).id : undefined);
    this.syncCursor(); // an unpinned tab shows the cursor node: make sure the session has it, also when no cursor has moved since the window opened
  }

  /** The "Add doc" flow for the node with this id (a sidebar item, or a card on the canvas). Resolves to whether a doc was added. */
  async addDocTo(key: string): Promise<boolean> {
    const structure = this.core.tree ? undefined : await this.core.loadStructure();
    const root = this.core.gitRoot ?? structure!.git_root;
    const node = findNode(this.core.tree!, key);
    if (!node || node.id === "/") throw new Error("That node is not in the structure any more.");
    const added = await addDoc((method, params) => this.core.request(method, params), root, node, node.docs);
    // the first attach in a workspace is when to offer the agents' files (07 §3): a favour, not awaited, and it never fails the attach
    if (added) void this.cli.offerAfterFirstAttach();
    return added;
  }

  /**
   * A definition's text and its baseline (docs/17-edit-in-canvas.md §4, §5), from VS Code's document (so unsaved edits show): the range's text, the version it was read at, the first
   * line's text before the range (the `def`'s own indentation, as written), the line ending, and whether the document has unsaved changes. A card is editable only while it does not:
   * its range comes from the last save and in a dirty file it can point at the wrong lines (E5). `saveFirst` saves it first (the card's Save button). The path comes from the webview,
   * so it must stay inside the opened folder.
   */
  async codeOf(relative: string, r: Range, saveFirst = false): Promise<CodeResult> {
    const doc = await this.documentIn(relative);
    if (saveFirst && doc.isDirty && !(await doc.save())) throw new Error(`${relative} could not be saved.`);
    return {
      text: doc.getText(new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character)),
      version: doc.version,
      prefix: doc.lineAt(Math.min(r.start.line, doc.lineCount - 1)).text.slice(0, r.start.character),
      eol: doc.eol === vscode.EndOfLine.CRLF ? "\r\n" : "\n",
      dirty: doc.isDirty,
    };
  }

  /**
   * Edit on a doc (§8): the same answer as for a function, with the whole file as the range and no prefix. A doc with unsaved changes in a tab is refused (E5), or saved first.
   * There is no range to refresh after the save, so "Save and Edit" answers the draft's start at once. The path is the Markdown file's own (`DocInfo.path`), checked by `Docs`.
   */
  async editStartDocOf(file: string, saveFirst: boolean): Promise<EditStartResult> {
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(this.docs.resolve(file)));
    if (doc.isDirty) {
      if (!saveFirst) return { dirty: true };
      if (!(await doc.save())) throw new Error(`${path.basename(file)} could not be saved.`);
    }
    const text = doc.getText();
    return { text, version: doc.version, prefix: "", eol: doc.eol === vscode.EndOfLine.CRLF ? "\r\n" : "\n", range: wholeRange(text) };
  }

  /**
   * The document a draft is of: a function's file below the opened folder, or, for a doc (§8), the Markdown file. A doc that cannot be opened (deleted or renamed since the draft
   * started) is `null`: the function is gone, as for a function the core cannot find (§7). A path that is not a V-NOC doc is an error, not "gone".
   */
  private async draftDocument(p: { kind?: "doc"; path: string }): Promise<vscode.TextDocument | null> {
    if (p.kind !== "doc") return this.documentIn(p.path);
    const file = this.docs.resolve(p.path);
    return vscode.workspace.openTextDocument(vscode.Uri.file(file)).then(
      (doc) => doc,
      () => null,
    );
  }

  /**
   * Save on a draft (docs/17-edit-in-canvas.md §6, E4). `applyEdit` carries no version and applies at any range, so the check is here, in three steps whose rules are
   * `saveDraft.ts`. The version and the text under the range against the draft's; if the version moved on, the core says where the function is now (`locate`, a
   * parameter so a test can pass one that edits the document before it answers) and the version is read again after it answers. **No `await` between the last
   * read and the call to `applyEdit`**: any await lets the document move on, and nothing after it would notice. A draft that was written already answers `saved`,
   * and writes nothing. The file is saved only if it was not dirty before: unsaved work of the user's own in a tab is never saved for them.
   *
   * §7: a draft that takes the function's `ID:` away asks first (`ask`, a parameter so a test can answer), in the row that writes and in no other. The question is the one
   * await before the write, so the document may have changed while it was open: after Save Anyway the rows run again from the top, reading the version again, and the
   * second pass does not ask again. Cancel answers `{ cancelled: true }` and writes nothing.
   */
  async editSaveOf(p: EditSaveParams, locate?: Locate, ask: AskToDropId = (key) => this.askToDropId(key)): Promise<EditSaveResult> {
    const doc = await this.draftDocument(p);
    if (doc === null) return { gone: true }; // §8: the `.md` was deleted or renamed
    // §8: a doc is all of one file, so the core has nothing to look for (the whole document is where it is), and a Markdown file has no ID line to take away
    const where: Locate = locate ?? (p.kind === "doc" ? docLocate : (key, document) => this.locateInCore(key, document));
    const written = reindent(p.text, p.base, p.prefix, p.eol);
    const vsRange = (r: Range) => new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character);
    const takesId = p.kind !== "doc" && dropsId(p.base, written);
    for (let asked = false; ; asked = true) {
      // from here to the first `await` (the core's answer, or the question) or to `applyEdit`: synchronous
      let at = p.range;
      const step = firstStep({ draftVersion: p.version, version: doc.version, current: doc.getText(vsRange(p.range)), base: p.base, written });
      let row: "write" | "already" | "conflict" | "gone";
      if (step !== "locate") row = step;
      else {
        const sent = doc.version;
        const found = await where(p.key, { uri: doc.uri.toString(), version: sent, text: doc.getText() });
        if (found) at = found;
        row = afterLocate({ found: found !== null, sentVersion: sent, version: doc.version, current: found ? doc.getText(vsRange(found)) : "", base: p.base, written });
      }
      if (row === "gone") return { gone: true };
      if (row === "conflict") return { conflict: true };
      if (row === "already") return answerForAlready(doc.isDirty) === "saved" ? { saved: true } : { written: true };
      if (takesId && !asked) {
        if (!(await ask(p.key))) return { cancelled: true };
        continue; // the document may have moved while the question was open: choose the row again
      }
      const wasDirty = doc.isDirty;
      const edit = new vscode.WorkspaceEdit();
      edit.replace(doc.uri, vsRange(at), written);
      const applied = await vscode.workspace.applyEdit(edit);
      const saved = applied && !wasDirty && (await doc.save());
      const outcome = outcomeAfterApply(applied, saved);
      return outcome === "conflict" ? { conflict: true } : outcome === "saved" ? { saved: true } : { written: true };
    }
  }

  /** The modal of §7: Save Anyway, or Cancel (VS Code adds it). Dismissing it is Cancel. */
  private async askToDropId(key: string): Promise<boolean> {
    const node = this.core.tree ? findNode(this.core.tree, key) : undefined;
    return (await vscode.window.showWarningMessage(idQuestion(key, node?.name), { modal: true }, SAVE_ANYWAY)) === SAVE_ANYWAY;
  }

  /** Where the core finds the function in this text, or null. */
  private async locateInCore(key: string, document: { uri: string; version: number; text: string }): Promise<Range | null> {
    const found = await this.core.request<{ range: Range } | null>("vnoc/locate", { key, document });
    return found?.range ?? null;
  }

  /**
   * Show Diff on a conflict: the file on the right, on the left the file with the draft in the function's place (where the core finds it now, else where the draft
   * started), in VS Code's diff editor. The left is a document in memory (`vnoc-draft:`): nothing is written.
   */
  async editDiffOf(p: EditDiffParams): Promise<void> {
    const doc = await this.draftDocument(p);
    if (doc === null) throw new Error(`${path.basename(p.path)} is not there any more.`);
    const found = p.kind === "doc" ? wholeRange(doc.getText()) : await this.locateInCore(p.key, { uri: doc.uri.toString(), version: doc.version, text: doc.getText() }).catch(() => null);
    const range = found ?? p.range;
    const from = doc.offsetAt(new vscode.Position(range.start.line, range.start.character));
    const to = doc.offsetAt(new vscode.Position(range.end.line, range.end.character));
    const left = this.draftDocs.add(doc.uri, withDraft(doc.getText(), from, to, reindent(p.text, p.base, p.prefix, p.eol)));
    const name = p.kind === "doc" ? path.basename(p.path) : p.key.slice(p.key.indexOf("::") + 2);
    await vscode.commands.executeCommand("vscode.diff", left, doc.uri, `${name}: your draft ↔ ${path.basename(p.path)}`);
  }

  /** The document of a file below the opened folder: the path comes from the webview, so it must stay inside it. */
  private async documentIn(relative: string): Promise<vscode.TextDocument> {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) throw new Error("Open a folder first.");
    const file = path.resolve(folder.uri.fsPath, relative);
    if (!file.startsWith(folder.uri.fsPath + path.sep)) throw new Error(`${relative} is outside the opened folder`);
    return vscode.workspace.openTextDocument(vscode.Uri.file(file));
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
