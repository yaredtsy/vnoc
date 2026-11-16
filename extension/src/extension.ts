import * as vscode from "vscode";

import type { EditSaveParams, Events, Notes, Range } from "@vnoc/shared/messages";
import type { ContextState, Intent } from "@vnoc/shared/session.ts";
import { AttentionView } from "./attentionView";
import { CallTrees } from "./callTree";
import { Commands, type Locate } from "./commands";
import { Core, watchFiles } from "./core";
import { DirtyFiles } from "./dirtyWatch";
import { DRAFT_SCHEME, DraftDocs } from "./draftDocs";
import { Docs } from "./docs";
import { coreStopped, facts, helpAndFeedback, reportIssue } from "./help";
import { LogTee } from "./report";
import { CANVAS, DOCS, Surfaces } from "./surfaces";
import { callLookup } from "@vnoc/shared/calls.ts";
import { createHandlers } from "./relay";
import { SessionStore } from "./session";
import { readingStarts, readingTheCode } from "./progress";
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
type SidebarRendered = { context: string; rows: number; selected: string | null; inView: boolean; drawn: boolean; height: number };
const sidebarPosts: SidebarRendered[] = [];
/** What the cards drew of their code (for the test suite). */
const codeShownPosts: { key: string; dirty: boolean; text: string }[] = [];
/** What the cards said about their edit state (for the test suite). */
const editStatePosts: { key: string; state: string; elsewhere: boolean }[] = [];
/** The answers of the pages to "check the keys of a code editor" (for the test suite). */
const keysCheckedPosts: { found: boolean; target: string; taken: string[] }[] = [];

/** What the bar's ⚠ last said (for the test suite). */
type Warned = { level: "none" | "warning" | "error"; text: string };
let warned: Warned | undefined;
/** The context chips the bar last drew (for the test suite). */
let chips: { id: string; label: string; active: boolean }[] = [];
const warnedWaiters: (() => void)[] = [];
/** What the bar's "unsaved drafts" chip last said (for the test suite): `count` 0 is no chip. */
let draftsChip = { count: 0, label: "" };

let core: Core | undefined;
/** Pages that crashed while drawing (for the test suite). */
const webviewErrors: string[] = [];

export function activate(ctx: vscode.ExtensionContext) {
  // the channel is wrapped so the last lines of the log can be copied into a bug report (the API cannot read a channel back)
  const output = new LogTee(vscode.window.createOutputChannel("V-NOC"));
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
  const c = new Core(ctx, output, indexChanged, (reason, canRestart) => void coreStopped(reason, canRestart ? () => c.restart() : undefined));
  const help = { ctx, log: output, coreVersion: () => c.version() };
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
  const draftDocs = new DraftDocs();
  const store = new SessionStore(ctx.workspaceState, () => c.tree, (call) => callLookup(callTrees.all())(call));
  const docs = new Docs(() => c.gitRoot, (name, data) => panels.emit(name, data));
  // the files with unsaved changes: the cards that show their code mark it (docs/17-edit-in-canvas.md step 9.1)
  const dirty = new DirtyFiles((paths) => panels.emit("dirtyFiles", { paths }));
  // what a webview may ask of the extension (`shared/messages.ts` lists it, `relay.ts` serves it)
  const handlers = createHandlers({
    // the first read has the progress line on the STRUCTURE view; a read after rows are drawn (the index changed) is quiet
    structure: () => {
      const read = async () => ({ ...(await c.loadStructure()), epoch: c.epoch });
      return c.tree === undefined ? readingTheCode(read) : read();
    },
    core: (method, params) => c.request(method, params),
    addDoc: (key) => commands.addDocTo(key),
    docs,
    code: (path, range) => commands.codeOf(path, range),
    editStart: (path, range, saveFirst) => commands.editStartOf(path, range, saveFirst),
    editSave: (params) => commands.editSaveOf(params),
    editDiff: (params) => commands.editDiffOf(params),
    copyText: async (text) => void (await vscode.env.clipboard.writeText(text)),
    goToCode: (key) => commands.goToCodeOf(key),
    dirtyFiles: () => dirty.paths,
    callTrees: () => callTrees.all(),
    openAt: (path, range) => commands.openAt(path, range),
    session: () => store.get(),
    pickCallSite: (sites) => commands.pickCallSite(sites),
    focusAttention: async () => void (await vscode.commands.executeCommand("vnoc.attention.focus")),
    showLog: async () => output.show(),
    reportIssue: async () => void (await vscode.commands.executeCommand("vnoc.reportIssue")),
    retryCalls: async (key) => callTrees.retry(key),
    rowClick: (context, id) => commands.rowClick(context, id),
    command: async (name) => void (await (name === "openFolder" ? vscode.commands.executeCommand("workbench.action.files.openFolder") : vscode.commands.executeCommand("vnoc.refreshStructure"))),
  });
  panels = new Surfaces(ctx, handlers, (note) => onNote(note, store, output, (keys, onScreen) => {
    callTrees.setDrawn(keys, onScreen); // what is on screen is probed, and the selected one is prefetched
    wantCalls();
  }));
  commands = new Commands(c, store, panels, async () => void (await vscode.commands.executeCommand("vnoc.structure.focus")), callTrees, draftDocs);

  const attentionTree = vscode.window.createTreeView("vnoc.attention", { treeDataProvider: attention });

  // Registered synchronously, before anything is awaited: on restart VS Code waits for these serializers (S1).
  ctx.subscriptions.push(
    vscode.window.registerWebviewPanelSerializer(CANVAS, { deserializeWebviewPanel: async (panel) => panels.restore("canvas", panel) }),
    vscode.window.registerWebviewPanelSerializer(DOCS, { deserializeWebviewPanel: async (panel) => panels.restore("docs", panel) }),
    // the STRUCTURE view: a webview view (registered here, synchronously, so a restored side bar finds its provider)
    registerSidebar(panels),
    attentionTree,
    ...commands.register(),
    vscode.commands.registerCommand("vnoc.reportIssue", () => reportIssue(help)),
    vscode.commands.registerCommand("vnoc.help", () => helpAndFeedback(help)),
    watchFiles(c, (fsPath) => docs.changed(fsPath)),
    docs.watchEditors(),
    dirty.watch(),
    vscode.workspace.registerTextDocumentContentProvider(DRAFT_SCHEME, draftDocs),
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
    /** what Report Issue fills the form with, and the log it would copy */
    reportFacts: () => facts(help),
    logTail: () => output.tail(),
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
    /** how many times the view's progress line was started: once, for the first read of the code */
    readingStarts: () => readingStarts(),
    /** whether the STRUCTURE view has been shown (its page exists) */
    sidebarOpen: () => panels.sidebar !== undefined,
    /** the context chips the bar draws now: empty with one context */
    chips: () => chips,
    /** the bar's "N unsaved drafts" chip as the page last drew it: `count` 0 and `label` "" when there is none */
    draftsChip: () => draftsChip,
    /** every drawing the pages reported, in order */
    renderedPosts: () => renderedPosts,
    /** the files with unsaved changes the host holds now (what the `dirtyFiles` event carries) */
    dirtyFiles: () => dirty.paths,
    /** a card's edit state each time it changed, in order: which card, the state, and whether it says "Already being edited in another card" */
    editStates: () => editStatePosts,
    /** a command for the canvas page that only the test suite sends: set a draft's text, press Save on a card (the suite cannot type or press keys in a webview) */
    viewCommand: (data: Events["viewCommand"]) => panels.emit("viewCommand", data),
    /** asks the canvas to dispatch keys at a code editor and say which the canvas took (`keysChecked` answers), and what it has said so far */
    checkKeys: () => panels.emit("viewCommand", { name: "checkKeys" }),
    keysChecked: () => keysCheckedPosts,
    /** the host's answer to Edit on a range (the request a card makes) */
    editStart: (path: string, range: Range, saveFirst = false) => commands.editStartOf(path, range, saveFirst),
    /** the host's answer to Save on a draft; `locate` stands in for the core's `vnoc/locate`, so a test can edit the document before it answers */
    editSave: (params: EditSaveParams, locate?: Locate) => commands.editSaveOf(params, locate),
    /** every time a card drew the code it read, in order: which card, whether it showed the unsaved mark, and the text */
    codeShown: () => codeShownPosts,
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
    case "keysChecked":
      keysCheckedPosts.push({ found: note.found, target: note.target, taken: note.taken });
      break;
    case "editState":
      editStatePosts.push({ key: note.key, state: note.state, elsewhere: note.elsewhere });
      break;
    case "codeShown":
      codeShownPosts.push({ key: note.key, dirty: note.dirty, text: note.text });
      break;
    case "sidebarRendered":
      sidebarPosts.push({ context: note.context, rows: note.rows, selected: note.selected, inView: note.inView, drawn: note.drawn, height: note.height });
      break;
    case "chipsDrawn":
      chips = note.chips;
      break;
    case "draftsDrawn":
      draftsChip = { count: note.count, label: note.label };
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
