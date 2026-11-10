import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { LanguageClient, TransportKind } from "vscode-languageclient/node";

import { addDoc } from "./addDoc";
import { Docs } from "./docs";
import { type FocusStack, jump, pop, prune, push } from "@vnoc/shared/focusStack";
import { FocusState } from "./focus";
import type { Notes, Range, StructureResult, TreeNode } from "@vnoc/shared/messages";
import { chainAt, docTarget, findNode, nodeAtPath } from "@vnoc/shared/tree";
import { CANVAS, DOCS, Panels } from "./panels";
import { type Handlers, createHandlers, event } from "./relay";
import { StructureView, absolute } from "./structureView";

// The core is a separate process: this extension starts it (vscode-languageclient is only a very good JSON-RPC-over-stdio
// client here, we use its custom-request API and none of the language features) and relays the webview's requests to it.
// The extension never starts a driver: the core owns the drivers.

let client: LanguageClient | undefined;
/** Resolves when the core is running. Everything that talks to it awaits this first (S1: never block activate()). */
let ready: Promise<void>;
let output: vscode.OutputChannel;

/** What the webview last said it drew (for the test suite). */
let rendered: { visible: number; hidden: number } | undefined;
const renderWaiters: ((r: { visible: number; hidden: number }) => void)[] = [];

/** Saves arrive in bursts (a format-on-save, a `git checkout`): collect the touched files for this long, then send one message. */
const CHANGE_DEBOUNCE_MS = 200;

/** A "Show on canvas" asked for while the canvas was closed: the new canvas takes it when it starts. (Focus needs no such thing: the canvas reads the shared focus stack.) */
let pendingReveal: string | undefined;
/** The node the docs tab shows (the one at the cursor), for a tab that is just starting. */
let docsTarget: string | undefined;
/** The pin in the docs tab is on: do not follow the cursor. */
let docsPinned = false;

/**
 * A binary path: the setting if set; else, when running from source (F5), the build folder of this repository
 * (`extension/` sits next to `target/` and `drivers/`); else `<extension>/bin/` (the installed VSIX).
 * From source the build folder wins, because `bin/` may hold a stale copy left by an earlier packaging run.
 * In Restricted Mode VS Code hands us the default instead of a workspace value (`restrictedConfigurations` in package.json),
 * so a cloned repo cannot choose which program we run.
 */
function binary(ctx: vscode.ExtensionContext, setting: string, name: string, devPath: string[]): string {
  const configured = vscode.workspace.getConfiguration("vnoc").get<string>(setting);
  if (configured) return configured;
  const exe = process.platform === "win32" ? `${name}.exe` : name;
  const fromSource = path.join(ctx.extensionPath, "..", ...devPath.slice(0, -1), exe);
  const packaged = path.join(ctx.extensionPath, "bin", exe);
  const developing = ctx.extensionMode === vscode.ExtensionMode.Development;
  return developing && fs.existsSync(fromSource) ? fromSource : fs.existsSync(packaged) ? packaged : fromSource;
}

async function startCore(ctx: vscode.ExtensionContext): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  const core = binary(ctx, "server.path", "vnoc", ["target", "debug", "vnoc"]);
  const driver = binary(ctx, "driver.path", "vnoc-driver-python", ["drivers", "python", "target", "debug", "vnoc-driver-python"]);
  if (!fs.existsSync(core)) {
    throw new Error(`The V-NOC core is not at ${core}. Build it: cargo build -p vnoc`);
  }
  client = new LanguageClient(
    "vnoc",
    "V-NOC core",
    { command: core, transport: TransportKind.stdio },
    // not a language server: no document selector. The driver's path and the opened folder travel to the core in `initializationOptions`.
    { documentSelector: [], outputChannel: output, initializationOptions: { driverPath: driver, root: folder?.uri.fsPath } },
  );
  client.onNotification("vnoc/indexChanged", (p: { revision: number }) => indexChanged(p.revision));
  await client.start();
  output.appendLine(`core started: ${core}\ndriver: ${driver}`);
}

async function request<T>(method: string, params?: object): Promise<T> {
  await ready;
  return client!.sendRequest<T>(method, params);
}

/** The last structure the core sent. The editor menus look the node at the cursor up here. */
let tree: TreeNode | undefined;
/** The absolute path of the git root. Node ids and the paths in `vnoc/attach` start there. */
let gitRoot: string | undefined;

/** The structure of the opened folder; the canvas, the docs tab and the sidebar all read it through here. It waits for the core's first build. */
async function loadStructure(): Promise<StructureResult> {
  if (!vscode.workspace.workspaceFolders?.[0]) throw new Error("Open a folder first: V-NOC shows the code of the folder you opened.");
  const structure = await request<StructureResult>("vnoc/structure");
  tree = structure.root;
  gitRoot = structure.git_root;
  // a focused node that no longer exists: drop it (and what was above it), so neither UI is left looking at nothing
  setFocus(prune(focus.get(), (id) => findNode(structure.root, id) !== undefined), "host");
  return structure;
}

/** The "Add doc" flow for the node with this id (a sidebar item, or a card on the canvas). Resolves to whether a doc was added. */
async function addDocTo(key: string): Promise<boolean> {
  const root = gitRoot ?? (await loadStructure()).git_root;
  const node = findNode(tree!, key);
  if (!node || node.id === "/") throw new Error("That node is not in the structure any more.");
  return addDoc(request, root, node, node.docs);
}

let sidebar: StructureView;
let sidebarView: vscode.TreeView<TreeNode>;
let panels: Panels;
let focus: FocusState;
const docs = new Docs(() => gitRoot, (name, data) => panels.emit(name, data));

/** The core has a new index: the sidebar redraws, and every open tab asks for the new structure. */
function indexChanged(revision: number) {
  refreshSidebar();
  panels.emit("indexChanged", { revision });
}

// ── focus: one stack for the canvas and the sidebar ────────────────────────────────────────────────────────────────────

/**
 * Both UIs show this stack: the canvas zooms into the top node, the sidebar tree starts at it. A change from the sidebar's buttons
 * (`from: "host"`) is sent to the canvas; a change that came from the canvas is already there.
 */
function setFocus(next: FocusStack, from: "host" | "canvas") {
  if (!focus.set(next)) return;
  showFocusInSidebar(next);
  if (from === "host") panels?.canvas?.webview.postMessage(event("focusStack", { stack: [...next] }));
}

/** Redraws the sidebar from the new structure, then the header (its names come from that structure). */
function refreshSidebar() {
  void sidebar.refresh().then(() => showFocusInSidebar(focus.get()));
}

/** The sidebar starts at the focused node, says so in its header, and shows Back and Clear (the `vnoc.focused` context key). */
function showFocusInSidebar(stack: FocusStack) {
  sidebar.setFocus(stack[stack.length - 1]);
  const names = stack.map((id) => sidebar.find(id)?.name ?? "?");
  sidebarView.description = names.length > 0 ? `Focus: ${names.join(" › ")}` : undefined;
  void vscode.commands.executeCommand("setContext", "vnoc.focused", stack.length > 0);
}

// ── the commands behind the editor's V-NOC menu, the sidebar's buttons and the canvas ──────────────────────────────────────

const isTreeNode = (arg: unknown): arg is TreeNode => typeof arg === "object" && arg !== null && "kind" in arg && "children" in arg;

/** The nodes around the cursor of the active editor: the file, then the class or function it is in (innermost last). */
async function chainAtCursor(): Promise<TreeNode[]> {
  const editor = vscode.window.activeTextEditor;
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!editor || !folder) return [];
  if (!tree) await loadStructure();
  const relative = path.relative(folder.uri.fsPath, editor.document.uri.fsPath).split(path.sep).join("/");
  return chainAt(tree!, relative, editor.selection.active.line);
}

/**
 * The node a command is about. It starts from one of: a sidebar item (a tree node), a card's right-click menu (`{ nodeKey }`), the Explorer
 * (a Uri, with the list of selected Uris as the next argument), or the editor's menu (a Uri, nothing after it: the node at the cursor).
 */
async function nodeFor(arg: unknown, selected?: unknown): Promise<TreeNode> {
  if (isTreeNode(arg)) return arg;
  if (!tree) await loadStructure();
  if (typeof arg === "object" && arg !== null && "nodeKey" in arg) {
    const hit = findNode(tree!, String((arg as { nodeKey: unknown }).nodeKey));
    if (!hit) throw new Error("That node is not in the structure any more.");
    return hit;
  }
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (arg instanceof vscode.Uri && Array.isArray(selected) && folder) {
    const hit = nodeAtPath(tree!, path.relative(folder.uri.fsPath, arg.fsPath).split(path.sep).join("/"));
    if (!hit) throw new Error("V-NOC has not read that file or folder (it is not Python, or it is ignored).");
    return hit;
  }
  const chain = await chainAtCursor();
  if (chain.length === 0) throw new Error("Put the cursor in a Python file that V-NOC has read.");
  return chain[chain.length - 1];
}

/** Runs a command and shows what went wrong as a message: menu clicks have nowhere else to report. */
const command = (run: (arg: unknown, selected?: unknown) => Promise<unknown> | unknown) => async (arg: unknown, selected?: unknown) => {
  try {
    await run(arg, selected);
  } catch (e) {
    void vscode.window.showErrorMessage(`V-NOC: ${(e as Error).message ?? e}`);
  }
};

/** Focus: narrow the canvas and the sidebar to this node, and bring the canvas up. */
function focusOn(id: string) {
  setFocus(push(focus.get(), id), "host");
  panels.showCanvas();
}

/** Show on canvas: reveal the node without changing the focus more than needed. An open canvas is told now; a canvas that is just starting asks for it. */
function revealOnCanvas(id: string) {
  const wasOpen = panels.canvas !== undefined;
  const panel = panels.showCanvas();
  if (wasOpen) panel.webview.postMessage(event("reveal", { id }));
  else pendingReveal = id;
}

/** Go to code: the definition in the editor (a folder is shown in the Explorer). */
async function goToCode(node: TreeNode) {
  const uri = absolute(node.path);
  if (node.kind === "folder") return vscode.commands.executeCommand("revealInExplorer", uri);
  const start = node.range ? new vscode.Position(node.range.start.line, node.range.start.character) : undefined;
  await vscode.window.showTextDocument(uri, start ? { selection: new vscode.Range(start, start) } : {});
}

/** Opens (or reuses) the docs tab beside the editor, showing the docs of the node at the cursor. */
async function showDocsBeside(arg: unknown, selected: unknown) {
  // from a card or a sidebar item: that node. From the editor's menu or the title bar: the node at the cursor.
  if (isTreeNode(arg) || (typeof arg === "object" && arg !== null && "nodeKey" in arg) || Array.isArray(selected)) {
    docsTarget = (await nodeFor(arg, selected)).id;
  } else {
    await updateDocsTarget(true);
  }
  const panel = panels.showDocs();
  if (docsTarget) panel.webview.postMessage(event("showNode", { id: docsTarget }));
}

/** The docs tab follows the cursor: the innermost node around it that has docs, else the innermost node. */
async function updateDocsTarget(force = false) {
  if (!force && (!panels.docs || docsPinned)) return;
  const target = docTarget(await chainAtCursor());
  if (!target || target.id === docsTarget) return;
  docsTarget = target.id;
  panels.docs?.webview.postMessage(event("showNode", { id: target.id }));
}

/** Cursor moves come in bursts; look at where it rests. */
const FOLLOW_DEBOUNCE_MS = 200;

function followCursor(): vscode.Disposable {
  let timer: NodeJS.Timeout | undefined;
  const soon = () => {
    clearTimeout(timer);
    timer = setTimeout(() => void updateDocsTarget().catch(() => {}), FOLLOW_DEBOUNCE_MS);
  };
  return vscode.Disposable.from(vscode.window.onDidChangeTextEditorSelection(soon), vscode.window.onDidChangeActiveTextEditor(soon), { dispose: () => clearTimeout(timer) });
}

/** Tells the core which files were touched. `.gitignore` is watched too: it changes which files are in scope. */
function watchFiles(): vscode.Disposable {
  // `.vnoc/**` and `.vnoc-id` are V-NOC's own files: a doc added by a teammate's `git pull` shows up like one added here
  const watcher = vscode.workspace.createFileSystemWatcher("**/{*.py,.gitignore,.vnoc-id,.vnoc/**}");
  const touched = new Set<string>();
  let timer: NodeJS.Timeout | undefined;
  const touch = (uri: vscode.Uri) => {
    docs.changed(uri.fsPath);
    touched.add(uri.fsPath);
    clearTimeout(timer);
    timer = setTimeout(() => {
      const paths = [...touched];
      touched.clear();
      void ready.then(() => client?.sendNotification("vnoc/filesChanged", { paths }));
    }, CHANGE_DEBOUNCE_MS);
  };
  return vscode.Disposable.from(watcher, watcher.onDidCreate(touch), watcher.onDidChange(touch), watcher.onDidDelete(touch), { dispose: () => clearTimeout(timer) });
}

export function activate(ctx: vscode.ExtensionContext) {
  output = vscode.window.createOutputChannel("V-NOC");
  ready = startCore(ctx);
  ready.catch((e) => {
    output.appendLine(String(e));
    vscode.window.showErrorMessage(String(e.message ?? e));
  });

  focus = new FocusState(ctx.workspaceState);
  panels = new Panels(ctx, handlers(), onNote);
  sidebar = new StructureView(async () => (await loadStructure()).root);
  sidebarView = vscode.window.createTreeView("vnoc.structure", { treeDataProvider: sidebar });
  showFocusInSidebar(focus.get()); // the focus saved with the workspace
  refreshSidebar();

  // Registered synchronously, before anything is awaited: on restart VS Code waits for this serializer (S1).
  ctx.subscriptions.push(
    vscode.commands.registerCommand("vnoc.openCanvas", () => panels.showCanvas()),
    vscode.window.registerWebviewPanelSerializer(CANVAS, { deserializeWebviewPanel: async (panel) => panels.restore("canvas", panel) }),
    vscode.window.registerWebviewPanelSerializer(DOCS, { deserializeWebviewPanel: async (panel) => panels.restore("docs", panel) }),
    sidebarView,
    // the tree follows the code by itself; Refresh is the way out if it ever does not: the core reads everything again
    vscode.commands.registerCommand("vnoc.refreshStructure", () => void ready.then(() => client?.sendNotification("vnoc/rebuild"))),
    watchFiles(),
    docs.watchEditors(),
    // these start from a sidebar item, a card's right-click menu, the Explorer, or the editor's menu (see `nodeFor`)
    vscode.commands.registerCommand("vnoc.focus", command(async (arg, selected) => focusOn((await nodeFor(arg, selected)).id))),
    vscode.commands.registerCommand("vnoc.focusBack", () => setFocus(pop(focus.get()), "host")),
    vscode.commands.registerCommand("vnoc.focusClear", () => setFocus(jump(focus.get(), 0), "host")),
    vscode.commands.registerCommand("vnoc.showOnCanvas", command(async (arg, selected) => revealOnCanvas((await nodeFor(arg, selected)).id))),
    vscode.commands.registerCommand("vnoc.goToCode", command(async (arg, selected) => goToCode(await nodeFor(arg, selected)))),
    vscode.commands.registerCommand("vnoc.showDocs", command(showDocsBeside)),
    vscode.commands.registerCommand("vnoc.addDoc", command(async (arg, selected) => {
      const node = await nodeFor(arg, selected);
      if (node.id === "/") throw new Error("Docs go on a folder inside the opened folder, not on the opened folder itself.");
      await addDocTo(node.id);
    })),
    followCursor(),
    { dispose: () => client?.stop() },
  );

  // What the test suite drives.
  return {
    ping: () => request<{ pong: boolean; pid: number; version: string }>("vnoc/ping"),
    structure: () => request<any>("vnoc/structure"),
    /** Resolves when a webview reports that it drew the structure (immediately if one already has). */
    waitRendered: (timeoutMs: number) =>
      new Promise<{ visible: number; hidden: number }>((resolve, reject) => {
        if (rendered) return resolve(rendered);
        const t = setTimeout(() => reject(new Error("the webview did not report a drawing")), timeoutMs);
        renderWaiters.push((r) => (clearTimeout(t), resolve(r)));
      }),
  };
}

/** The text of a definition, from VS Code's document (so unsaved edits show). The path comes from the webview, so it must stay inside the opened folder. */
async function readSlice(relative: string, r: Range): Promise<string> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) throw new Error("Open a folder first.");
  const file = path.resolve(folder.uri.fsPath, relative);
  if (!file.startsWith(folder.uri.fsPath + path.sep)) throw new Error(`${relative} is outside the opened folder`);
  const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
  return doc.getText(new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character));
}

/** What a webview may ask of the extension (`shared/messages.ts` lists it, `relay.ts` serves it). */
function handlers(): Handlers {
  return createHandlers({
    structure: loadStructure,
    core: request,
    addDoc: addDocTo,
    docs,
    code: readSlice,
    goToCode: async (key) => {
      if (!tree) await loadStructure();
      const node = findNode(tree!, key);
      if (!node) throw new Error("That node is not in the structure any more.");
      await goToCode(node);
    },
    takePendingReveal: () => {
      const id = pendingReveal;
      pendingReveal = undefined;
      return id;
    },
    focusStack: () => [...focus.get()],
    docsTarget: () => docsTarget ?? null,
  });
}

/** What a webview says that needs no answer. */
function onNote(note: Notes) {
  switch (note.type) {
    case "rendered":
      rendered = { visible: note.visible, hidden: note.hidden };
      output.appendLine(`webview drew ${note.visible} nodes (${note.hidden} hidden)`);
      renderWaiters.splice(0).forEach((w) => w(rendered!));
      break;
    case "focusChanged":
      setFocus(note.stack, "canvas");
      break;
    case "pin":
      docsPinned = note.pinned;
      if (!note.pinned) void updateDocsTarget().catch(() => {});
      break;
    case "close":
      break; // the panel closes itself (panels.ts)
  }
}

export async function deactivate() {
  await client?.stop();
}
