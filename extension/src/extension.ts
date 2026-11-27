import * as vscode from "vscode";

import { languageExtensions } from "@vnoc/shared/languages.ts";
import type { Draft, EditSaveParams, EditStartParams, Events, LanguageInfo, Notes, Page, Range } from "@vnoc/shared/messages";
import type { ContextState, Intent } from "@vnoc/shared/session.ts";
import { AttentionView } from "./attentionView";
import { CallTrees } from "./callTree";
import { type AskToDropId, Commands, type Locate } from "./commands";
import { CliSetup } from "./cliSetup";
import { Core, watchFiles } from "./core";
import { DraftCopy, type Notify, showNotification } from "./draftCopy";
import { DirtyFiles } from "./dirtyWatch";
import { DRAFT_SCHEME, DraftDocs } from "./draftDocs";
import { Docs } from "./docs";
import { coreStopped, facts, helpAndFeedback, reportIssue } from "./help";
import { LogTee } from "./report";
import { CANVAS, DOCS, Surfaces } from "./surfaces";
import { callLookup } from "@vnoc/shared/calls.ts";
import { createHandlers } from "./relay";
import { SessionStore } from "./session";
import { Tours, tourContext, vnocIsAbove } from "./tours";
import { stoppable } from "@vnoc/shared/tourWriting.ts";
import { type Level, TourActions } from "./tourActions";
import * as path from "path";
import { readingStarts, readingTheCode } from "./progress";
import { registerSidebar } from "./sidebarView";
import { moveAllToTrash, moveToTrash, restoreEntry } from "./trash";

/** How long after the extension starts it checks for drafts a closed canvas left: the tabs of the window are read by then, and a canvas tab VS Code restores is not mistaken for a missing one. */
const START_CHECK_MS = 1500;

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
/** The answers the STRUCTURE view drew for its search box (for the test suite). */
const searchDrawnPosts: { query: string; ids: string[]; docs: string[]; total: number; notes: string[]; ms: number; trip: number }[] = [];
/** What the canvas said it dimmed for the sidebar's search (for the test suite). */
const searchDimPosts: { query: string; dimmed: string[] }[] = [];
/** What the docs tab said it shows (for the test suite). */
const docPanelPosts: { open: boolean; node: string | null }[] = [];
const docsShownPosts: { pinned: string | null; node: string | null; doc: string | null }[] = [];
/** What the STRUCTURE view said when it handled `focusSearch`: whether the cursor is in the box, and what the box holds (for the test suite). */
const searchFocusedPosts: { focused: boolean; text: string }[] = [];
/** What the tour's stage drew, each time it changed (for the test suite). */
const stagePosts: Extract<Notes, { type: "stageDrawn" }>[] = [];
/** What the tour's map drew, each time it changed (for the test suite). */
const mapPosts: Extract<Notes, { type: "mapDrawn" }>[] = [];
/** What the bar said in a tour context (for the test suite). */
const barPosts: Extract<Notes, { type: "tourBar" }>[] = [];
/** The viewport each canvas last reported, by context (for the test suite). */
const viewports: Record<string, { x: number; y: number; zoom: number }> = {};
/** What the **Commit and Pin…** dialog was, each time it changed (for the test suite). */
const pinDialogPosts: Extract<Notes, { type: "pinDialog" }>[] = [];
/** What the **Ask for a Tour** dialog was, each time it changed (for the test suite). */
const askDialogPosts: Extract<Notes, { type: "askDialog" }>[] = [];
/** What the sidebar's TOURS section drew, each time it changed (for the test suite). */
const toursPosts: Extract<Notes, { type: "toursDrawn" }>[] = [];
/** The nodes that showed `▶ N`, by surface, as each page last said (for the test suite). */
const tourChipPosts: Extract<Notes, { type: "tourChips" }>[] = [];
/** What the cards drew of their code (for the test suite). */
const codeShownPosts: { key: string; dirty: boolean; text: string }[] = [];
/** What the cards said about their edit state (for the test suite). */
const editStatePosts: { key: string; state: string; elsewhere: boolean; mark: string | null }[] = [];
/** The answers of the pages to "check the keys of a code editor" (for the test suite). */
const keysCheckedPosts: { found: boolean; target: string; taken: string[]; hosts: number }[] = [];

/** What the bar's ⚠ last said (for the test suite). */
type Warned = { level: "none" | "warning" | "error"; text: string };
let warned: Warned | undefined;
/** The context chips the bar last drew (for the test suite). */
let chips: { id: string; label: string; active: boolean; dot?: "pulsing" | "still" }[] = [];
const warnedWaiters: (() => void)[] = [];
/** What the bar's "unsaved drafts" chip last said (for the test suite): the canvas page's last post, and `count` 0 (no chip) while no canvas page is open. */
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
    tours.indexChanged(); // `▶ N` is by the structure's keys
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
  // the host's copy of the canvas's drafts: fed by the page, acted on only when the canvas is closed or at start (docs/17 E6, §7)
  const tabOf = (viewType: string) => vscode.window.tabGroups.all.flatMap((g) => g.tabs).some((t) => t.input instanceof vscode.TabInputWebview && t.input.viewType.endsWith(viewType));
  const draftCopy = new DraftCopy(ctx.workspaceState, {
    save: (params) => commands.editSaveOf(params),
    reopen: (page) => void (page === "canvas" ? panels.showCanvas() : panels.showDocs()),
    pageOpen: (page) => (page === "canvas" ? panels.canvas !== undefined || tabOf(CANVAS) : panels.docs !== undefined || tabOf(DOCS)),
  });
  // the doc panel's pencil (§8): a doc the other page has a draft of is `busy`, and the host is the only one that can say so
  const editStart = (p: EditStartParams) => (draftCopy.busy(p.path, p.page) ? Promise.resolve({ busy: true as const }) : commands.editStartDocOf(p.path, p.saveFirst === true));
  const store = new SessionStore(ctx.workspaceState, () => c.tree, (call) => callLookup(callTrees.all())(call));
  // the tours: the host asks the core, holds the open tour with its place, and sets the tour context's selection from it (`tours.ts`)
  // the status bar item (design 04 §6): `▶ Tour 3/10` while a tour is open, and a click shows the canvas with the tour active
  const tourItem = vscode.window.createStatusBarItem("vnoc.tour", vscode.StatusBarAlignment.Left, 50);
  tourItem.name = "V-NOC Tour";
  tourItem.command = "vnoc.tourStatus";
  tourItem.tooltip = "The tour: show it, stop writing it, or stop playing it";
  // the one notification (W11): a tour started being written and the canvas is not showing a tour. Nothing opens by itself. The suite cannot click a button of a notification
  const noticed: { id: string; open: () => Promise<void> }[] = [];
  let statusText: string | null = null;
  const tours = new Tours({
    request: (method, params) => c.request(method, params as object | undefined),
    statusBar: (text) => {
      statusText = text;
      if (text === null) tourItem.hide();
      else {
        tourItem.text = text;
        tourItem.show();
      }
    },
    announce: (row) => {
      const open = () => commands.openTourById(row.id, row.title);
      noticed.push({ id: row.id, open });
      void vscode.window.showInformationMessage(`A tour is being written: “${row.title ?? row.id}”.`, "Open Tour").then((pick) => (pick === "Open Tour" ? open() : undefined));
    },
    canvasVisible: () => panels.canvas?.visible === true,
    // W11: the canvas is visible and no tour is open: the tour opens at its cover. The suite sees it in the session
    open: (row) => void commands.openTourById(row.id, row.title),
    session: () => store.get(),
    dispatch: (intent, context) => store.dispatch(intent, context),
    emit: (name, data) => panels.emit(name, data),
    vnocAbove: () => vnocIsAbove(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath, c.gitRoot),
    sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
    every: (ms, run) => {
      const timer = setInterval(run, ms);
      return { stop: () => clearInterval(timer) };
    },
    log: (text) => output.appendLine(text),
  });
  // what the user does to a tour from a menu or a button: the messages say what the core answered (`tourActions.ts`). The suite cannot click a message's button or a QuickPick: it queues
  // the answers (`answerTourMessages`, `answerTourPicks`), and reads what was said
  const messagesSaid: { level: Level; text: string; buttons: string[] }[] = [];
  let messageAnswers: (string | undefined)[] | null = null;
  let pickAnswers: string[] | null = null;
  let inputAnswers: (string | undefined)[] | null = null;
  let fileAnswers: (string | undefined)[] | null = null;
  const tourActions = new TourActions({
    request: (method, params) => c.request(method, params as object | undefined),
    view: (id) => tours.get(id),
    dirtyFiles: () => dirty.paths,
    save: async (paths) => {
      const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "";
      for (const p of paths) {
        const doc = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === path.join(folder, p));
        // a save that did not happen (the file changed on disk meanwhile) stops it: the IDs are written into the file on disk, and the buffer would then disagree with it
        if (doc && !(await doc.save())) throw new Error(`${p} could not be saved, so no ID was written. Save it (or revert it), then write the IDs.`);
      }
    },
    say: async (level, text, buttons) => {
      messagesSaid.push({ level, text, buttons });
      if (messageAnswers !== null) return messageAnswers.shift();
      const show = level === "info" ? vscode.window.showInformationMessage : level === "warn" ? vscode.window.showWarningMessage : vscode.window.showErrorMessage;
      return show(text, ...buttons);
    },
    pick: async (items, options) => {
      if (pickAnswers !== null) {
        const want = pickAnswers.shift();
        return items.find((i) => i.label === want);
      }
      return vscode.window.showQuickPick(items, { title: options.title, placeHolder: options.placeHolder, ignoreFocusOut: true });
    },
    // the dialog is the canvas page's: the host tells it, and the canvas comes up if it was not open
    openDialog: (id, title) => {
      tours.askDialog(id, title);
      void panels.showCanvas();
    },
    showChanges: () => void vscode.commands.executeCommand("workbench.view.scm"),
    input: async (options) => (inputAnswers !== null ? inputAnswers.shift() : vscode.window.showInputBox({ title: options.title, prompt: options.prompt, value: options.value, ignoreFocusOut: true })),
    // a tour that went to the trash cannot stay open: its context closes
    closeIfOpen: (id) => {
      const open = tourContext(store.get());
      if (open && open.tour === id) store.dispatch({ type: "closeContext", id: open.id }, open.id);
    },
    deleted: (ids, tokens, words) => panels.emit("tourDeleted", { ids, tokens, words }),
    refreshList: () => void tours.list().catch((e) => output.appendLine(`vnoc/tours: ${String((e as Error).message ?? e)}`)),
    // the files of **Export as JSON…** and **Import a Tour from JSON…**: the suite cannot answer a native dialog, it queues the path (`answerTourFiles`: undefined is a dismissed dialog)
    saveJson: async (suggested, text) => {
      const folder = vscode.workspace.workspaceFolders?.[0]?.uri;
      const chosen = fileAnswers !== null ? fileAnswers.shift() : (await vscode.window.showSaveDialog({ title: "Export the Tour as JSON", defaultUri: folder && vscode.Uri.joinPath(folder, suggested), filters: { JSON: ["json"] } }))?.fsPath;
      if (chosen === undefined) return undefined;
      await vscode.workspace.fs.writeFile(vscode.Uri.file(chosen), new TextEncoder().encode(text));
      return chosen;
    },
    openJson: async (id, text) => {
      const doc = await vscode.workspace.openTextDocument({ language: "json", content: text });
      jsonTabs.set(doc.uri.toString(), { id, doc });
      await vscode.window.showTextDocument(doc, { preview: false, ...(panels.canvas?.active ? { viewColumn: vscode.ViewColumn.Beside } : {}) });
      void markJsonTab();
    },
    pickJson: async () => {
      const chosen = fileAnswers !== null ? fileAnswers.shift() : (await vscode.window.showOpenDialog({ title: "Import a Tour from JSON", canSelectMany: false, filters: { JSON: ["json"] } }))?.[0]?.fsPath;
      if (chosen === undefined) return undefined;
      const uri = vscode.Uri.file(chosen);
      return { target: uri.toString(), text: new TextDecoder().decode(await vscode.workspace.fs.readFile(uri)) };
    },
    showRefusals: async (target, refusals) => {
      const doc = jsonTabs.get(target)?.doc ?? (await vscode.workspace.openTextDocument(vscode.Uri.parse(target)));
      await vscode.window.showTextDocument(doc, { preview: false, ...(panels.canvas?.active ? { viewColumn: vscode.ViewColumn.Beside } : {}) });
      refusalMarks.set(
        doc.uri,
        refusals.map((r) => {
          // the line's own text, not its indentation
          const line = doc.lineAt(Math.max(0, Math.min((r.line ?? 1) - 1, doc.lineCount - 1)));
          const diagnostic = new vscode.Diagnostic(new vscode.Range(line.lineNumber, line.firstNonWhitespaceCharacterIndex, line.lineNumber, line.text.length), r.words, vscode.DiagnosticSeverity.Error);
          diagnostic.source = "V-NOC";
          diagnostic.code = r.place;
          return diagnostic;
        }),
      );
    },
    clearRefusals: (target) => refusalMarks.delete(vscode.Uri.parse(target)),
    checkAll: async () => void (await vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title: "Checking tours…" }, () => tours.checkAll())),
    copy: (text) => Promise.resolve(vscode.env.clipboard.writeText(text)),
  });
  /** the untitled tabs **Open as JSON** made: each is the document of one tour, which **Import into This Tour** (on the tab's title bar) imports back into it */
  const jsonTabs = new Map<string, { id: string; doc: vscode.TextDocument }>();
  const refusalMarks = vscode.languages.createDiagnosticCollection("V-NOC tour");
  ctx.subscriptions.push(refusalMarks);
  /** the title bar's **Import into This Tour** is on the tabs of `jsonTabs` only: `vnoc.tourJson` says whether the active editor is one */
  const markJsonTab = () => vscode.commands.executeCommand("setContext", "vnoc.tourJson", jsonTabs.has(vscode.window.activeTextEditor?.document.uri.toString() ?? ""));
  ctx.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor(() => void markJsonTab()),
    vscode.workspace.onDidCloseTextDocument((doc) => {
      jsonTabs.delete(doc.uri.toString());
      refusalMarks.delete(doc.uri);
    }),
    // a refusal points at a line of the text as it was imported: an edit makes the marks stale, so they go
    vscode.workspace.onDidChangeTextDocument((e) => e.contentChanges.length > 0 && refusalMarks.delete(e.document.uri)),
  );
  /** The tour a row's menu was opened on (`tour` of what the row said), or a stop's tour */
  const tourOfArg = (arg: unknown): string => {
    const id = (arg as { tour?: unknown } | null)?.tour;
    if (typeof id !== "string") throw new Error("Pick a tour first.");
    return id;
  };
  const docs = new Docs(() => c.gitRoot, (name, data) => panels.emit(name, data));
  // the files with unsaved changes: the cards that show their code mark it (docs/17-edit-in-canvas.md step 9.1)
  const dirty = new DirtyFiles((paths) => panels.emit("dirtyFiles", { paths }));
  // **Ask for a Tour…** is a dialog of the canvas page: the host tells it where it was chosen, and the canvas comes up if it was not open
  const openAsk = (from: { key?: string; tour?: string }) => {
    tours.askForm(from);
    void panels.showCanvas();
  };
  // what a webview may ask of the extension (`shared/messages.ts` lists it, `relay.ts` serves it)
  const handlers = createHandlers({
    openAsk,
    // the first read has the progress line on the STRUCTURE view; a read after rows are drawn (the index changed) is quiet
    structure: () => {
      const read = async () => ({ ...(await c.loadStructure()), epoch: c.epoch });
      return c.tree === undefined ? readingTheCode(read) : read();
    },
    core: (method, params) => c.request(method, params),
    addDoc: (key) => commands.addDocTo(key),
    docs,
    code: (path, range, saveFirst) => commands.codeOf(path, range, saveFirst),
    editStart,
    editSave: (params) => commands.editSaveOf(params),
    editDiff: (params) => commands.editDiffOf(params),
    copyText: async (text) => void (await vscode.env.clipboard.writeText(text)),
    goToCode: (key) => commands.goToCodeOf(key),
    dirtyFiles: () => dirty.paths,
    drafts: (page) => draftCopy.drafts(page),
    callTrees: () => callTrees.all(),
    openAt: (path, range) => commands.openAt(path, range),
    session: () => store.get(),
    pickCallSite: (sites) => commands.pickCallSite(sites),
    focusAttention: async () => void (await vscode.commands.executeCommand("vnoc.attention.focus")),
    showLog: async () => output.show(),
    reportIssue: async () => void (await vscode.commands.executeCommand("vnoc.reportIssue")),
    retryCalls: async (key) => callTrees.retry(key),
    rowClick: (context, id) => commands.rowClick(context, id),
    showTours: (key, context) => commands.showToursOf(key, context),
    tourActions,
    openTour: async () => void (await vscode.commands.executeCommand("vnoc.openTour")),
    openChange: async (file) => {
      const root = c.gitRoot ?? (await c.loadStructure()).git_root;
      const uri = vscode.Uri.file(path.join(root, file));
      // the diff against HEAD, as the source control view's own click opens it; where the git extension is not there, the file itself
      if ((await vscode.commands.getCommands()).includes("git.openChange")) await vscode.commands.executeCommand("git.openChange", uri);
      else await vscode.commands.executeCommand("vscode.open", uri);
    },
    showDoc: (params) => commands.showDoc(params),
    command: async (name) => void (await (name === "openFolder" ? vscode.commands.executeCommand("workbench.action.files.openFolder") : vscode.commands.executeCommand("vnoc.refreshStructure"))),
    tours,
  });
  panels = new Surfaces(
    ctx,
    handlers,
    (note) =>
      onNote(note, store, output, (keys, onScreen) => {
        callTrees.setDrawn(keys, onScreen); // what is on screen is probed, and the selected one is prefetched
        wantCalls();
      }, (page, drafts) => draftCopy.replace(page, drafts), () => void panels.showCanvas()),
    (page) => {
      if (page === "canvas") draftsChip = { count: 0, label: "" }; // the chip it drew is gone with the page, which posts nothing on its way out
      void draftCopy.onPageClosed(page);
    },
    () => {
      // the box's page is gone and cannot say its box is empty: the canvas undims and nothing is pointed at
      store.dispatch({ type: "search", query: null });
      store.dispatch({ type: "highlight", id: null });
    },
  );
  const cli = new CliSetup(ctx, c, output);
  commands = new Commands(c, store, panels, async () => void (await vscode.commands.executeCommand("vnoc.structure.focus")), callTrees, draftDocs, docs, cli);

  // drafts a canvas left in a window that was reloaded or closed (§7): asked once, a moment after the start
  const startCheck = setTimeout(() => void draftCopy.atStart(), START_CHECK_MS);

  const attentionTree = vscode.window.createTreeView("vnoc.attention", { treeDataProvider: attention });

  // Registered synchronously, before anything is awaited: on restart VS Code waits for these serializers (S1).
  ctx.subscriptions.push(
    tourItem,
    // a click on the status bar item: the canvas, with the tour that is open active
    // a click on the status bar item (TR23a): what can be done to the open tour; **Show Tour** is the click it was
    vscode.commands.registerCommand("vnoc.tourStatus", async () => {
      const open = tourContext(store.get());
      if (!open) return;
      const held = tours.heldTour();
      const writing = held !== null && "stops" in held && stoppable(held);
      const chosen = await tourActions.statusPick(writing);
      if (chosen === "show") await vscode.commands.executeCommand("vnoc.showTour");
      else if (chosen === "stopWriting" && open.tour) await tours.halt(open.tour).catch((e) => output.appendLine(`vnoc/tourHalt: ${String((e as Error).message ?? e)}`));
      else if (chosen === "stopPlaying") store.dispatch({ type: "closeContext", id: open.id }, open.id);
    }),
    vscode.commands.registerCommand("vnoc.showTour", async () => {
      const open = tourContext(store.get());
      if (open) store.dispatch({ type: "activate", id: open.id }, open.id);
      await panels.showCanvas();
    }),
    vscode.window.registerWebviewPanelSerializer(CANVAS, { deserializeWebviewPanel: async (panel) => panels.restore("canvas", panel) }),
    vscode.window.registerWebviewPanelSerializer(DOCS, { deserializeWebviewPanel: async (panel) => panels.restore("docs", panel) }),
    // the STRUCTURE view: a webview view (registered here, synchronously, so a restored side bar finds its provider)
    registerSidebar(panels),
    attentionTree,
    ...commands.register(),
    // the tour row's menu (`tourRow`): the arg is what the row said, `{ tour }`; a refusal is shown in the core's words by the action itself
    // the row's first three entries (05 §3): the click, the cover again, and MAIN on the tour's root
    vscode.commands.registerCommand("vnoc.tourOpen", (arg) => commands.openTourById(tourOfArg(arg), (arg as { title?: string | null }).title ?? undefined)),
    vscode.commands.registerCommand("vnoc.tourStartAgain", async (arg) => {
      const id = tourOfArg(arg);
      await tours.go(id, "cover");
      await commands.openTourById(id, (arg as { title?: string | null }).title ?? undefined);
    }),
    vscode.commands.registerCommand("vnoc.tourRevealRoot", async (arg) => {
      const view = await tours.get(tourOfArg(arg));
      await commands.revealTourRoot("stops" in view ? (view.stops[0]?.key ?? null) : null);
    }),
    vscode.commands.registerCommand("vnoc.tourPin", (arg) => tourActions.pin(tourOfArg(arg))),
    vscode.commands.registerCommand("vnoc.tourCommitAndPin", (arg) => tourActions.commitAndPin(tourOfArg(arg))),
    vscode.commands.registerCommand("vnoc.tourWriteIds", (arg) => tourActions.writeIds(tourOfArg(arg))),
    vscode.commands.registerCommand("vnoc.tourRename", (arg) => tourActions.rename(tourOfArg(arg))),
    vscode.commands.registerCommand("vnoc.tourShare", (arg) => tourActions.share(tourOfArg(arg), true)),
    vscode.commands.registerCommand("vnoc.tourMakePrivate", (arg) => tourActions.share(tourOfArg(arg), false)),
    vscode.commands.registerCommand("vnoc.tourDelete", (arg) => tourActions.delete(tourOfArg(arg))),
    vscode.commands.registerCommand("vnoc.tourSettings", () => tourActions.settings()),
    vscode.commands.registerCommand("vnoc.tourExportJson", (arg) => tourActions.exportJson(tourOfArg(arg))),
    vscode.commands.registerCommand("vnoc.tourOpenJson", (arg) => tourActions.openJson(tourOfArg(arg))),
    vscode.commands.registerCommand("vnoc.tourCopyId", (arg) => tourActions.copyId(tourOfArg(arg))),
    vscode.commands.registerCommand("vnoc.tourImportInto", () => {
      const doc = vscode.window.activeTextEditor?.document;
      const tab = doc && jsonTabs.get(doc.uri.toString());
      if (!doc || !tab) throw new Error("Open a tour as JSON first: its title bar has Import into This Tour.");
      return tourActions.importInto(tab.id, doc.getText(), doc.uri.toString());
    }),
    vscode.commands.registerCommand("vnoc.tourImportFile", () => tourActions.importFile()),
    vscode.commands.registerCommand("vnoc.tourCheckAll", () => tourActions.checkAll()),
    // **Ask for a Tour…**: the node it was chosen on (a menu names it; the palette and the buttons mean the selected one), else none; a row's **Ask for a New Tour** names the tour
    vscode.commands.registerCommand("vnoc.tourAsk", async (arg, selected) => openAsk({ key: await commands.askKeyOf(arg, selected) })),
    vscode.commands.registerCommand("vnoc.tourAskAgain", (arg) => openAsk({ tour: tourOfArg(arg) })),
    // **Show Docs** in a stop's menu: the docs panel, as the bar's `ⓘ` opens it
    vscode.commands.registerCommand("vnoc.tourShowDocs", async () => {
      await panels.showCanvas();
      panels.emit("viewCommand", { name: "showDocPanel" });
    }),
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
      tours.sync(); // a tour became this window's tour context, or its context closed
    }),
    // a tour's file changed, or another process moved a place: the open tour and the list are asked for again, or the place is taken
    c.onToursChanged(({ ids }) => tours.toursChanged(ids)),
    c.onTourPlaceChanged(({ id, place }) => tours.placeChanged(id, place)),
    // what the session points at may be gone (a focused node you deleted: you are one step back, not looking at nothing)
    c.onStructure((structure) => {
      if (attention.update(structure)) {
        void vscode.commands.executeCommand("setContext", "vnoc.hasAttention", attention.hasAttention);
        void vscode.commands.executeCommand("setContext", "vnoc.hasMissing", attention.hasMissing);
      }
      store.dispatch({ type: "treeChanged" });
      wantCalls();
      commands.syncCursor();
      tours.recheck(); // the git root is known now: is `.vnoc/` above the opened folder?
    }),
    // every call tree came from the old core, and its revisions mean nothing against the new one: drop them all, fetch what is open again
    c.onRestarted(() => {
      callTrees.restarted();
      tours.restarted(); // the new core does not know a window shows a tour
    }),
    { dispose: () => tours.dispose() }, // the core is told the tour is not shown any more, before it is stopped
    { dispose: () => void c.stop() },
    { dispose: () => clearTimeout(startCheck) },
  );
  tours.sync(); // a tour context the session was restored with
  tours.start(); // the list, so that a tour that appears later is known as new

  // the command in PATH follows this version, and `vnoc.cliInstalled` and `vnoc.agentsSetUp` are set (they wait for the core: they never hold up the start)
  void cli.activate();

  // The files whose menus carry the V-NOC submenu, and those whose drivers have call trees (Show Calls is offered only there): both come from the core,
  // which says them once it has read its drivers' handshakes, and again after a restart. No file is a V-NOC file until then.
  let contexts = { extensions: [] as string[], callTreeExtensions: [] as string[] };
  const setExtensions = (languages: LanguageInfo[]) => {
    const { all, callTree } = languageExtensions(languages);
    contexts = { extensions: all, callTreeExtensions: callTree };
    void vscode.commands.executeCommand("setContext", "vnoc.extensions", all);
    void vscode.commands.executeCommand("setContext", "vnoc.callTreeExtensions", callTree);
  };
  setExtensions([]);
  ctx.subscriptions.push(c.onLanguages(setExtensions));

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
    /** the command line's setup: the core binary the extension runs, the context keys it set, and the one-time offer (the suite asks without showing it) */
    cli: {
      binaryPath: () => c.binaryPath,
      driverPaths: () => c.driverPaths,
      context: (key: string) => cli.context(key),
      decideOffer: () => cli.decideOffer(),
      resetOffer: () => cli.resetOffer(),
      offerAfterFirstAttach: () => cli.offerAfterFirstAttach(),
    },
    /** the extensions the menus' context keys `vnoc.extensions` and `vnoc.callTreeExtensions` hold now */
    languageContexts: () => contexts,
    /** the run of the core: one more after each restart */
    coreEpoch: () => c.epoch,
    /** what the tour holder holds: the open tour's view as the pages were last sent it (null: none open), and the plain list */
    tours: { open: () => tours.heldTour(), list: () => tours.list() },
    /** the status bar item's text (null: hidden), the tours the notification was shown for, and a click on its **Open Tour** */
    tourStatus: () => statusText,
    toursNoticed: () => noticed.map((n) => n.id),
    toursNoticeOpen: (id: string) => noticed.find((n) => n.id === id)?.open(),
    /** every drawing of the tour's stage, in order: the step it shows, its words and its code, which lines are lit */
    stageDrawn: () => stagePosts,
    /** a key pressed in the tour's stage, and a click on one of its buttons (`next`, `prev`, `goToCode`, `start`, `closeTour`), as the page handles them: the suite cannot press keys or click in a webview */
    stageKey: (key: string) => panels.emit("viewCommand", { name: "stageKey", key }),
    stageClick: (button: string) => panels.emit("viewCommand", { name: "stageClick", key: button }),
    /** every drawing of the tour's map, in order: its cards (badge, ring, call line) and its heavy edges */
    mapDrawn: () => mapPosts,
    /** what the bar said in a tour context each time it changed: the crumbs, `moved by the agent`, whether `←` is enabled */
    tourBar: () => barPosts,
    /** the viewport each canvas last reported, by context id */
    viewports: () => viewports,
    /** a click on a stop's card of the tour's map */
    mapClick: (stop: string) => panels.emit("viewCommand", { name: "mapClick", key: stop }),
    /** sets the pan and zoom of the canvas that is on screen, as a drag and the wheel would: `x,y,zoom` */
    setView: (text: string) => panels.emit("viewCommand", { name: "setView", text }),
    /** every state of the **Ask for a Tour** dialog, in order (`dialog: null`: closed) */
    askDialog: () => askDialogPosts,
    /** a control of the ask dialog, as the page handles it: `question` (with `text`), `agent` (the box), `depth` (with `text`), `detail` (with `text`), `about:remove`, `copy` (⌘Enter), `cancel` (Esc) */
    askDialogClick: (key: string, text?: string) => panels.emit("viewCommand", { name: "askDialog", key, text }),
    /** every state of the **Commit and Pin…** dialog, in order (`dialog: null`: closed) */
    pinDialog: () => pinDialogPosts,
    /** a button of the dialog, as the page handles it: `commit`, `cancel`, `always`, `message` (with `text`), `file:<path>` */
    pinDialogClick: (key: string, text?: string) => panels.emit("viewCommand", { name: "pinDialog", key, text }),
    /** every message the tour actions said (`level`, its `text`, its `buttons`), in order; with answers queued the message is not shown, and each is answered with the next one (`undefined`: dismissed) */
    tourMessages: () => messagesSaid,
    answerTourMessages: (answers: (string | undefined)[] | null) => void (messageAnswers = answers),
    /** the QuickPicks of Tour Settings…, answered by the label of the item to pick; `null`: VS Code's own */
    answerTourPicks: (labels: string[] | null) => void (pickAnswers = labels),
    answerTourInputs: (answers: (string | undefined)[] | null) => void (inputAnswers = answers),
    /** the files of the two JSON dialogs, by their path (`undefined`: a dismissed dialog); `null`: VS Code's own */
    answerTourFiles: (paths: (string | undefined)[] | null) => void (fileAnswers = paths),
    /** **Open Tour…**'s QuickPick: the label to pick (`null`: VS Code's own), and every list it showed (a line that starts with `— ` is a separator) */
    answerOpenTour: (labels: string[] | null) => void (commands.tourPick.pickAnswers = labels),
    openTourPicks: () => commands.tourPick.picksShown,
    /** every state of the canvas's doc panel (the bar's `ⓘ`), in order */
    docPanel: () => docPanelPosts,
    /** every drawing of the sidebar's TOURS section, in order */
    toursDrawn: () => toursPosts,
    /** every report of the nodes that show `▶ N` (the cards of the canvas, the rows of the structure that are drawn): the node's id, its count and the chip's tooltip */
    tourChips: () => tourChipPosts,
    /** a click on a node's `▶ N`, as the page's request makes it (the same function as the node menu's **Show Tours**) */
    showTours: (key: string, context: string) => commands.showToursOf(key, context),
    /** a click in the TOURS section (`fold`, `view:all`, `view:on`, `viewkey:<Arrow>`, `showAll`, `unfold`, `row:<tour>`, `stop:<id>`, `step:<id>`, `back`, `close`, `key:<Key>`), as the page handles it */
    toursClick: (what: string) => panels.emit("viewCommand", { name: "toursClick", key: what }),
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
    /** whether VS Code says the STRUCTURE view is on screen now (a page that is not is throttled: its timers and drawing wait) */
    sidebarVisible: () => panels.sidebar?.visible === true,
    /** the context chips the bar draws now: empty with one context */
    chips: () => chips,
    /** the bar's "N unsaved drafts" chip as the canvas page last drew it: `count` 0 and `label` "" when there is none, and while no canvas page is open */
    draftsChip: () => draftsChip,
    /** every drawing the pages reported, in order */
    renderedPosts: () => renderedPosts,
    /** the host's copy of the canvas's drafts now (what the page's `drafts` notes left, and the flows on close and at start changed) */
    draftCopy: () => draftCopy.all,
    /** one page's part of the copy */
    draftCopyOf: (page: Page) => draftCopy.drafts(page),
    /** what a page's `drafts` note does: that page's list in the copy is replaced */
    feedDrafts: (page: Page, drafts: Draft[]) => draftCopy.replace(page, drafts),
    /** the request the doc panel's pencil makes, with the busy rule */
    editStartRequest: (params: EditStartParams) => editStart(params),
    /** how the copy shows its notifications; a test answers them. No argument: VS Code's own */
    setDraftNotify: (notify?: Notify) => void (draftCopy.notify = notify ?? showNotification),
    /** the check made when the extension starts, run now */
    draftsAtStart: () => draftCopy.atStart(),
    /** the files with unsaved changes the host holds now (what the `dirtyFiles` event carries) */
    dirtyFiles: () => dirty.paths,
    /** a card's edit state each time it changed, in order: which card, the state, whether it says it is being edited elsewhere, and which line is above its code */
    editStates: () => editStatePosts,
    /** a command for the canvas page that only the test suite sends: set a draft's text, press Save on a card (the suite cannot type or press keys in a webview) */
    viewCommand: (data: Events["viewCommand"]) => panels.emit("viewCommand", data),
    /** asks the canvas to dispatch keys at a code editor and say which the canvas took (`keysChecked` answers, with the number of code areas it had: `hosts`), and what it has said so far */
    checkKeys: () => panels.emit("viewCommand", { name: "checkKeys" }),
    keysChecked: () => keysCheckedPosts,
    /** the host's answer to a card's code (the request a card makes): the text and the baseline of a range */
    codeOf: (path: string, range: Range, saveFirst = false) => commands.codeOf(path, range, saveFirst),
    /** the host's answer to Save on a draft; `locate` stands in for the core's `vnoc/locate`, so a test can edit the document before it answers, and `ask` for the modal about a draft that takes the ID away */
    editSave: (params: EditSaveParams, locate?: Locate, ask?: AskToDropId) => commands.editSaveOf(params, locate, ask),
    /** every answer the STRUCTURE view drew for its search box, in order: the text it answers, the rows' ids, how many matched, and the time from the change of the box (`ms`) and from the request (`trip`) to the draw */
    searchDrawn: () => searchDrawnPosts,
    /** puts text in the STRUCTURE view's search box as typing would (the view command reaches the sidebar through `panels.emit`, like every other page) */
    setSearch: (text: string) => panels.emit("viewCommand", { name: "setSearch", text }),
    /** every answer the canvas applied for the sidebar's search, in order: the text, and the ids of the cards it dimmed; `dimmed: []` (and no text) when the search ended */
    searchDim: () => searchDimPosts,
    /** what the docs tab showed each time it changed, in order: the node it is pinned to, the node it draws, and the doc's path */
    docsShown: () => docsShownPosts,
    /** every `focusSearch` the STRUCTURE view handled (*V-NOC: Search…*), in order: was the cursor in the box, and what did it hold */
    searchFocused: () => searchFocusedPosts,
    /** a key pressed in the STRUCTURE view's search box (`ArrowDown`, `Enter`, `Escape`), as the page handles it: the suite cannot press keys in a webview */
    searchKey: (key: string) => panels.emit("viewCommand", { name: "searchKey", key }),
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
function onNote(note: Notes, store: SessionStore, output: vscode.OutputChannel, cardsDrawn: (keys: string[], onScreen: string[]) => void, drafts: (page: Page, drafts: Draft[]) => void, showCanvas: () => void) {
  switch (note.type) {
    case "intent":
      store.dispatch(note.intent, note.context);
      if (note.intent.type === "tourOpen") showCanvas(); // a click on a tour's row is the request to read it: the canvas comes up beside the editor
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
    case "mapDrawn":
      mapPosts.push(note);
      break;
    case "tourBar":
      barPosts.push(note);
      break;
    case "viewport":
      viewports[note.context] = { x: note.x, y: note.y, zoom: note.zoom };
      break;
    case "toursDrawn":
      toursPosts.push(note);
      break;
    case "tourChips":
      tourChipPosts.push(note);
      break;
    case "stageDrawn":
      stagePosts.push(note);
      break;
    case "pinDialog":
      pinDialogPosts.push(note);
      break;
    case "askDialog":
      askDialogPosts.push(note);
      break;
    case "keysChecked":
      keysCheckedPosts.push({ found: note.found, target: note.target, taken: note.taken, hosts: note.hosts });
      break;
    case "editState":
      editStatePosts.push({ key: note.key, state: note.state, elsewhere: note.elsewhere, mark: note.mark });
      break;
    case "codeShown":
      codeShownPosts.push({ key: note.key, dirty: note.dirty, text: note.text });
      break;
    case "searchDrawn":
      searchDrawnPosts.push({ query: note.query, ids: note.ids, docs: note.docs, total: note.total, notes: note.notes, ms: note.ms, trip: note.trip });
      break;
    case "searchDim":
      searchDimPosts.push({ query: note.query, dimmed: note.dimmed });
      break;
    case "docPanel":
      docPanelPosts.push({ open: note.open, node: note.node });
      break;
    case "docsShown":
      docsShownPosts.push({ pinned: note.pinned, node: note.node, doc: note.doc });
      break;
    case "searchFocused":
      searchFocusedPosts.push({ focused: note.focused, text: note.text });
      break;
    case "sidebarRendered":
      sidebarPosts.push({ context: note.context, rows: note.rows, selected: note.selected, inView: note.inView, drawn: note.drawn, height: note.height });
      break;
    case "chipsDrawn":
      chips = note.chips;
      break;
    case "drafts":
      drafts(note.page, note.drafts);
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
