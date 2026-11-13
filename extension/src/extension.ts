import * as vscode from "vscode";

import type { Notes } from "@vnoc/shared/messages";
import type { ContextState, Intent } from "@vnoc/shared/session.ts";
import { AttentionView } from "./attentionView";
import { CallTrees } from "./callTree";
import { Commands } from "./commands";
import { Core, watchFiles } from "./core";
import { CursorMark } from "./cursorMark";
import { Docs } from "./docs";
import { FocusView } from "./focusView";
import { CANVAS, DOCS, Panels } from "./panels";
import { callLookup } from "@vnoc/shared/calls.ts";
import { createHandlers } from "./relay";
import { SessionStore } from "./session";
import { type Row, StructureView } from "./structureView";
import { moveAllToTrash, moveToTrash, restoreEntry } from "./trash";

// Wires the pieces together: the core, the session, the two webview tabs, the sidebar, the commands. Nothing here decides anything.

/** What the webview last said it drew (for the test suite). */
type Rendered = { visible: number; hidden: number; calls: number };
let rendered: Rendered | undefined;
/** Every drawing a webview reported, in order (for the test suite). */
const renderedPosts: Rendered[] = [];
const renderWaiters: ((r: Rendered) => void)[] = [];

/** What the bar's ⚠ last said (for the test suite). */
type Warned = { level: "none" | "warning" | "error"; text: string };
let warned: Warned | undefined;
const warnedWaiters: (() => void)[] = [];

let core: Core | undefined;
/** Pages that crashed while drawing (for the test suite). */
const webviewErrors: string[] = [];

export function activate(ctx: vscode.ExtensionContext) {
  const output = vscode.window.createOutputChannel("V-NOC");
  let panels: Panels;
  let sidebar: StructureView;
  let sidebarView: vscode.TreeView<Row>;
  const focusView = new FocusView();
  const cursorMark = new CursorMark();
  const attention = new AttentionView();
  let commands: Commands;

  /** The core has a new index: the sidebar redraws, and every open tab asks for the new structure. */
  const indexChanged = (revision: number) => {
    refreshSidebar();
    callTrees.indexChanged(revision); // stale trees are fetched again 800 ms after the last change
    panels.emit("indexChanged", { revision });
  };
  /** What the session has open or selected, or a tree that arrived, changes which call trees are wanted. */
  const wantCalls = () => {
    if (c.tree) callTrees.update(store.get(), c.tree);
  };
  /** Redraws the sidebar from the new structure, then the header (its names come from that structure). */
  const refreshSidebar = () => void sidebar.refresh().then(showFocusInSidebar);
  /** The tree starts at the focused node (with the focus bar above it), and the FOCUS view lists every step. */
  const showFocusInSidebar = () => {
    const stack = store.get().contexts[store.get().active].focus;
    sidebar.setFocus(stack);
    focusView.update(stack, (id) => sidebar.find(id));
  };
  /** The row of the selected node, selected in the tree: both surfaces show the same node. (`reveal` fires the selection event; `reduce` returns the same session for it, so nothing loops.) */
  const revealSelection = () => {
    const id = store.get().contexts[store.get().active].selected;
    const row = id ? sidebar.rowForId(id) : undefined;
    if (row && sidebarView.visible) void Promise.resolve(sidebarView.reveal(row, { select: true, focus: false })).catch(() => {});
  };

  const c = new Core(ctx, output, indexChanged);
  core = c;
  // the call trees first: the session's lookup of what a call id points at reads them
  const callTrees = new CallTrees(c, {
    tree: (key, result) => {
      panels.emit("callTree", { key, result });
      store.dispatch({ type: "callsChanged", owner: key }); // what no longer resolves in the new tree is dropped from the session
      wantCalls(); // a cut call node may now want its callee's tree
    },
    state: (key, state, message) => panels.emit("callTreeState", { key, state, message }),
    reset: () => panels.emit("callTreesReset", {}),
  });
  const store = new SessionStore(ctx.workspaceState, () => c.tree, (call) => callLookup(callTrees.all())(call));
  const docs = new Docs(() => c.gitRoot, (name, data) => panels.emit(name, data));
  // what a webview may ask of the extension (`shared/messages.ts` lists it, `relay.ts` serves it)
  const handlers = createHandlers({
    structure: async () => ({ ...(await c.loadStructure()), epoch: c.epoch }),
    core: (method, params) => c.request(method, params),
    addDoc: (key) => commands.addDocTo(key),
    docs,
    code: (path, range) => commands.codeOf(path, range),
    goToCode: (key) => commands.goToCodeOf(key),
    callTrees: () => callTrees.all(),
    openAt: (path, range) => commands.openAt(path, range),
    session: () => store.get(),
    focusAttention: async () => void (await vscode.commands.executeCommand("vnoc.attention.focus")),
    showLog: async () => output.show(),
  });
  panels = new Panels(ctx, handlers, (note) => onNote(note, store, output));
  commands = new Commands(c, store, panels, async (id) => {
    const row = sidebar.rowForId(id);
    if (row) await sidebarView.reveal(row, { select: true, focus: true, expand: false }); // shows the V-NOC side bar too
  }, callTrees);

  sidebar = new StructureView(async () => (await c.loadStructure()).root);
  sidebarView = vscode.window.createTreeView("vnoc.structure", { treeDataProvider: sidebar, showCollapseAll: true });
  const focusTree = vscode.window.createTreeView("vnoc.focusStack", { treeDataProvider: focusView });
  const attentionTree = vscode.window.createTreeView("vnoc.attention", { treeDataProvider: attention });
  showFocusInSidebar(); // the focus saved with the workspace

  // Registered synchronously, before anything is awaited: on restart VS Code waits for these serializers (S1).
  ctx.subscriptions.push(
    vscode.window.registerWebviewPanelSerializer(CANVAS, { deserializeWebviewPanel: async (panel) => panels.restore("canvas", panel) }),
    vscode.window.registerWebviewPanelSerializer(DOCS, { deserializeWebviewPanel: async (panel) => panels.restore("docs", panel) }),
    sidebarView,
    focusTree,
    attentionTree,
    vscode.window.registerFileDecorationProvider(cursorMark),
    // the sidebar's four states: loading and error say so in the view, empty and error also show the welcome text (package.json `viewsWelcome`)
    sidebar.onDidChangeState((state) => {
      sidebarView.message = state.kind === "loading" ? "Reading the code…" : state.kind === "error" ? state.message : undefined;
      void vscode.commands.executeCommand("setContext", "vnoc.sidebarState", state.kind);
    }),
    // a click or the arrow keys on a row select it; `select` never opens anything (opening is `vnoc.open`'s job)
    sidebarView.onDidChangeSelection((e) => {
      const row = e.selection[0];
      if (row?.row === "node") store.dispatch({ type: "select", id: row.node.id });
    }),
    sidebarView.onDidChangeVisibility((e) => e.visible && revealSelection()),
    ...commands.register(),
    watchFiles(c, (fsPath) => docs.changed(fsPath)),
    docs.watchEditors(),
    // every change of the session goes to the open webviews, whole; the sidebar redraws its header when the focus moved
    store.onChange(({ prev, next }) => {
      panels.emit("session", next);
      wantCalls();
      if (prev.contexts[prev.active].focus !== next.contexts[next.active].focus) showFocusInSidebar();
      if (prev.contexts[prev.active].selected !== next.contexts[next.active].selected) revealSelection();
      if (prev.cursor !== next.cursor) {
        cursorMark.update(next.cursor);
        sidebar.setCursor(next.cursor);
      }
    }),
    // what the session points at may be gone (a focused node you deleted: you are one step back, not looking at nothing)
    c.onStructure((structure) => {
      if (attention.update(structure)) {
        void vscode.commands.executeCommand("setContext", "vnoc.hasAttention", attention.hasAttention);
        void vscode.commands.executeCommand("setContext", "vnoc.hasMissing", attention.hasMissing);
      }
      // the command is offered only when the driver that read the code has the method
      void vscode.commands.executeCommand("setContext", "vnoc.callTree", structure.capabilities?.callTree === true);
      store.dispatch({ type: "treeChanged" });
      wantCalls();
      commands.syncCursor();
    }),
    // every call tree came from the old core, and its revisions mean nothing against the new one: drop them all, fetch what is open again
    c.onRestarted(() => callTrees.restarted()),
    { dispose: () => void c.stop() },
  );

  // the files whose menus carry the V-NOC submenu (later: from the driver manifests)
  void vscode.commands.executeCommand("setContext", "vnoc.extensions", [".py", ".pyi"]);
  refreshSidebar(); // last: the listeners above hear the "loading" state

  // What the test suite drives.
  return {
    session: () => store.get(),
    webviewErrors: () => webviewErrors,
    /** what the next window restart would read back */
    saved: () => ctx.workspaceState.get<{ contexts: Record<string, ContextState> }>("vnoc.session.v1"),
    /** the rows of NEEDS ATTENTION: the Missing IDs and the Problems */
    attention: () => attention.snapshot(),
    /** the call trees the host holds, by entry key */
    callTrees: () => callTrees.all(),
    /** what happened to the call trees, newest last: states, trees, drops, resets */
    callTreeLog: () => callTrees.log,
    /** makes every call tree answer at least this slowly, so a test can cancel a request before it is made */
    holdCallTrees: (ms: number) => callTrees.hold(ms),
    /** the run of the core: one more after each restart */
    coreEpoch: () => c.epoch,
    /** an intent, as a click on a card would send it */
    dispatch: (intent: Intent, context?: string) => store.dispatch(intent, context),
    /** a request to the core (the suite attaches docs through it, without the input boxes) */
    request: <T>(method: string, params?: object) => c.request<T>(method, params),
    /** Remove and Restore without the questions (the commands add those) */
    moveToTrash: (id: string) => moveToTrash(c, id),
    moveAllToTrash: (ids: string[]) => moveAllToTrash(c, ids),
    restoreEntry: (entry: string) => restoreEntry(c, entry),
    ping: () => c.request<{ pong: boolean; pid: number; version: string }>("vnoc/ping"),
    structure: () => c.request<any>("vnoc/structure"),
    /** selects a node's row in the sidebar tree, as a click would (the session hears it from `onDidChangeSelection`) */
    selectInTree: async (id: string) => {
      const row = sidebar.rowForId(id);
      if (row) await sidebarView.reveal(row, { select: true, focus: false });
    },
    /** every drawing the pages reported, in order */
    renderedPosts: () => renderedPosts,
    /** stops the core process, as a crash would: the next request to it fails. `restartCore` brings it back. */
    breakCore: () => c.stop(),
    /** starts it again, so the suites after the one that broke it still have a core */
    restartCore: () => c.restart(),
    /** tells the open pages the index changed, so each reads the structure again (what a save does) */
    emitIndexChanged: () => panels.emit("indexChanged", { revision: 0 }),
    /** resolves when the bar's ⚠ says something that satisfies `ready` (immediately if it already does) */
    waitWarned: (ready: (w: Warned) => boolean, timeoutMs: number) =>
      new Promise<Warned>((resolve, reject) => {
        const t = setTimeout(() => reject(new Error(`the bar's warning never said the expected thing (last: ${JSON.stringify(warned)})`)), timeoutMs);
        const check = () => {
          if (warned !== undefined && ready(warned)) return clearTimeout(t), resolve(warned);
          warnedWaiters.push(check);
        };
        check();
      }),
    /** Resolves when a webview reports that it drew the structure (immediately if one already has). */
    waitRendered: (timeoutMs: number) =>
      new Promise<Rendered>((resolve, reject) => {
        if (rendered) return resolve(rendered);
        const t = setTimeout(() => reject(new Error("the webview did not report a drawing")), timeoutMs);
        renderWaiters.push((r) => (clearTimeout(t), resolve(r)));
      }),
  };
}

/** What a webview says that needs no answer. */
function onNote(note: Notes, store: SessionStore, output: vscode.OutputChannel) {
  switch (note.type) {
    case "intent":
      store.dispatch(note.intent, note.context);
      break;
    case "rendered":
      rendered = { visible: note.visible, hidden: note.hidden, calls: note.calls };
      renderedPosts.push(rendered);
      output.appendLine(`webview drew ${note.visible} nodes (${note.hidden} hidden, ${note.calls} with calls)`);
      renderWaiters.splice(0).forEach((w) => w(rendered!));
      break;
    case "warned":
      warned = { level: note.level, text: note.text };
      warnedWaiters.splice(0).forEach((w) => w());
      break;
    case "error":
      webviewErrors.push(note.message);
      output.appendLine(`a V-NOC page crashed while drawing: ${note.message}`);
      break;
    case "close":
      break; // the panel closes itself (panels.ts)
  }
}

export async function deactivate() {
  await core?.stop();
}
