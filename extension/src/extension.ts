import * as vscode from "vscode";

import type { Notes } from "@vnoc/shared/messages";
import type { ContextState, Intent } from "@vnoc/shared/session.ts";
import { AttentionView } from "./attentionView";
import { CallTrees } from "./callTree";
import { Commands } from "./commands";
import { Core, watchFiles } from "./core";
import { Docs } from "./docs";
import { CANVAS, DOCS, Surfaces } from "./surfaces";
import { callLookup } from "@vnoc/shared/calls.ts";
import { createHandlers } from "./relay";
import { SessionStore } from "./session";
import { registerSidebar } from "./sidebarView";
import { moveAllToTrash, moveToTrash, restoreEntry } from "./trash";

// Wires the pieces together: the core, the session, the webview tabs and the STRUCTURE view, NEEDS ATTENTION, the commands. Nothing here decides anything.

/** What the webview last said it drew (for the test suite). */
type Rendered = { context: string; visible: number; hidden: number; callCards: number; chevrons: number };
let rendered: Rendered | undefined;
/** Every drawing a webview reported, in order (for the test suite). */
const renderedPosts: Rendered[] = [];
const renderWaiters: ((r: Rendered) => void)[] = [];

/** What the STRUCTURE view last said it drew (for the test suite). */
type SidebarRendered = { rows: number; selected: string | null; inView: boolean; contexts: number; height: number };
const sidebarPosts: SidebarRendered[] = [];

/** What the bar's ⚠ last said (for the test suite). */
type Warned = { level: "none" | "warning" | "error"; text: string };
let warned: Warned | undefined;
/** The context chips the bar last drew (for the test suite). */
let chips: { id: string; label: string; active: boolean }[] = [];
const warnedWaiters: (() => void)[] = [];

let core: Core | undefined;
/** Pages that crashed while drawing (for the test suite). */
const webviewErrors: string[] = [];

export function activate(ctx: vscode.ExtensionContext) {
  const output = vscode.window.createOutputChannel("V-NOC");
  let panels: Surfaces;
  const attention = new AttentionView();
  let commands: Commands;

  /** The core has a new index: every open page (the tabs and the STRUCTURE view) asks for the new structure. */
  const indexChanged = (revision: number) => {
    callTrees.indexChanged(revision); // stale trees are fetched again 800 ms after the last change
    panels.emit("indexChanged", { revision });
  };
  /** What the session has open or selected, or a tree that arrived, changes which call trees are wanted. */
  const wantCalls = () => {
    if (c.tree) callTrees.update(store.get(), c.tree);
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
    probe: (key, count, revision, epoch) => panels.emit("callProbe", { key, count, revision, epoch }),
    probeDropped: (key) => panels.emit("callProbeDropped", { key }),
    state: (key, state, message) => panels.emit("callTreeState", { key, state, message }),
    dropped: (key) => panels.emit("callTreeDropped", { key }),
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
    pickCallSite: (sites) => commands.pickCallSite(sites),
    focusAttention: async () => void (await vscode.commands.executeCommand("vnoc.attention.focus")),
    showLog: async () => output.show(),
    retryCalls: async (key) => callTrees.retry(key),
    rowClick: (context, id) => commands.rowClick(context, id),
    command: async (name) => void (await (name === "openFolder" ? vscode.commands.executeCommand("workbench.action.files.openFolder") : vscode.commands.executeCommand("vnoc.refreshStructure"))),
  });
  panels = new Surfaces(ctx, handlers, (note) => onNote(note, store, output, (keys, onScreen) => {
    callTrees.setDrawn(keys, onScreen); // what is on screen is probed, and the selected one is prefetched
    wantCalls();
  }));
  commands = new Commands(c, store, panels, async () => void (await vscode.commands.executeCommand("vnoc.structure.focus")), callTrees);

  const attentionTree = vscode.window.createTreeView("vnoc.attention", { treeDataProvider: attention });

  // Registered synchronously, before anything is awaited: on restart VS Code waits for these serializers (S1).
  ctx.subscriptions.push(
    vscode.window.registerWebviewPanelSerializer(CANVAS, { deserializeWebviewPanel: async (panel) => panels.restore("canvas", panel) }),
    vscode.window.registerWebviewPanelSerializer(DOCS, { deserializeWebviewPanel: async (panel) => panels.restore("docs", panel) }),
    // the STRUCTURE view: a webview view (registered here, synchronously, so a restored side bar finds its provider)
    registerSidebar(panels),
    attentionTree,
    ...commands.register(),
    watchFiles(c, (fsPath) => docs.changed(fsPath)),
    docs.watchEditors(),
    // every change of the session goes to the open webviews, whole: the canvas, the docs tab and the STRUCTURE view each draw what they need of it
    store.onChange(({ next }) => {
      panels.emit("session", next);
      wantCalls();
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
    /** the probes held (how many call cards opening each function would show), by key */
    callProbes: () => callTrees.probes(),
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
    /** a click on a sidebar row, as the page sends it (the click rule of 07 §4) */
    rowClick: (context: string, id: string) => commands.rowClick(context, id),
    /** every drawing the STRUCTURE view reported, in order; empty until the view was shown */
    sidebarPosts: () => sidebarPosts,
    /** whether the STRUCTURE view has been shown (its page exists) */
    sidebarOpen: () => panels.sidebar !== undefined,
    /** the context chips the bar draws now: empty with one context */
    chips: () => chips,
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
function onNote(note: Notes, store: SessionStore, output: vscode.OutputChannel, cardsDrawn: (keys: string[], onScreen: string[]) => void) {
  switch (note.type) {
    case "intent":
      store.dispatch(note.intent, note.context);
      break;
    case "rendered":
      rendered = { context: note.context, visible: note.visible, hidden: note.hidden, callCards: note.callCards, chevrons: note.chevrons };
      renderedPosts.push(rendered);
      output.appendLine(`webview drew ${note.visible} nodes (${note.hidden} hidden, ${note.callCards} call cards)`);
      renderWaiters.splice(0).forEach((w) => w(rendered!));
      break;
    case "cardsDrawn":
      cardsDrawn(note.keys, note.onScreen);
      break;
    case "sidebarRendered":
      sidebarPosts.push({ rows: note.rows, selected: note.selected, inView: note.inView, contexts: note.contexts, height: note.height });
      break;
    case "chipsDrawn":
      chips = note.chips;
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
