import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { LanguageClient, TransportKind } from "vscode-languageclient/node";

import { addDoc } from "./addDoc";
import { Docs } from "./docs";
import { chainAt, docTarget } from "./nodeAt";
import { CANVAS, DOCS, Panels } from "./panels";
import { StructureView, type TreeNode } from "./structureView";

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

/** Something the canvas should do as soon as it is up: a Focus or Show on canvas asked for while it was closed. */
let pendingOnCanvas: { type: "focus" | "reveal"; id: string } | undefined;
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

interface Structure {
  root: TreeNode;
  git_root: string;
}

/** The last structure the core sent. The editor menus look the node at the cursor up here. */
let tree: TreeNode | undefined;
/** The absolute path of the git root. Node ids and the paths in `vnoc/attach` start there. */
let gitRoot: string | undefined;

/** The structure of the opened folder; the canvas, the docs tab and the sidebar all read it through here. It waits for the core's first build. */
async function loadStructure(): Promise<Structure> {
  if (!vscode.workspace.workspaceFolders?.[0]) throw new Error("Open a folder first: V-NOC shows the code of the folder you opened.");
  const structure = await request<Structure>("vnoc/structure");
  tree = structure.root;
  gitRoot = structure.git_root;
  return structure;
}

/** The "Add doc" flow for the node with this id (a sidebar item, or a card on the canvas). Resolves to whether a doc was added. */
async function addDocTo(key: string, existingDocs: number): Promise<boolean> {
  const root = gitRoot ?? (await loadStructure()).git_root;
  return addDoc(request, root, key, existingDocs);
}

let sidebar: StructureView;
let panels: Panels;
const docs = new Docs(() => gitRoot, (message) => panels.post(message));

/** The core has a new index: the sidebar redraws, and every open tab asks for the new structure. */
function indexChanged(revision: number) {
  void sidebar.refresh();
  panels.post({ type: "indexChanged", revision });
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

/** The node a command is about: the tree item or card it was started from, else the node at the cursor. */
async function nodeFor(arg: unknown): Promise<TreeNode> {
  if (isTreeNode(arg)) return arg;
  const chain = await chainAtCursor();
  if (chain.length === 0) throw new Error("Put the cursor in a Python file that V-NOC has read.");
  return chain[chain.length - 1];
}

/** Runs a command and shows what went wrong as a message: menu clicks have nowhere else to report. */
const command = (run: (arg: unknown) => Promise<unknown> | unknown) => async (arg: unknown) => {
  try {
    await run(arg);
  } catch (e) {
    void vscode.window.showErrorMessage(`V-NOC: ${(e as Error).message ?? e}`);
  }
};

/** Focus (zoom into the node) or Show (reveal it, keep the focus): an open canvas is told now; a canvas that is just starting asks for it. */
function onCanvas(type: "focus" | "reveal", id: string) {
  const wasOpen = panels.canvas !== undefined;
  const panel = panels.showCanvas();
  if (wasOpen) panel.webview.postMessage({ type, id });
  else pendingOnCanvas = { type, id };
}

/** Opens (or reuses) the docs tab beside the editor, showing the docs of the node at the cursor. */
async function showDocsBeside() {
  await updateDocsTarget(true);
  panels.showDocs();
}

/** The docs tab follows the cursor: the innermost node around it that has docs, else the innermost node. */
async function updateDocsTarget(force = false) {
  if (!force && (!panels.docs || docsPinned)) return;
  const target = docTarget(await chainAtCursor());
  if (!target || target.id === docsTarget) return;
  docsTarget = target.id;
  panels.docs?.webview.postMessage({ type: "showNode", id: target.id });
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

  panels = new Panels(ctx, answer, {
    rendered(visible, hidden) {
      rendered = { visible, hidden };
      output.appendLine(`webview drew ${visible} nodes (${hidden} hidden)`);
      renderWaiters.splice(0).forEach((w) => w(rendered!));
    },
    pinned(pinned) {
      docsPinned = pinned;
      if (!pinned) void updateDocsTarget().catch(() => {});
    },
  });
  sidebar = new StructureView(async () => (await loadStructure()).root);
  void sidebar.refresh();

  // Registered synchronously, before anything is awaited: on restart VS Code waits for this serializer (S1).
  ctx.subscriptions.push(
    vscode.commands.registerCommand("vnoc.openCanvas", () => panels.showCanvas()),
    vscode.window.registerWebviewPanelSerializer(CANVAS, { deserializeWebviewPanel: async (panel) => panels.restore("canvas", panel) }),
    vscode.window.registerWebviewPanelSerializer(DOCS, { deserializeWebviewPanel: async (panel) => panels.restore("docs", panel) }),
    vscode.window.registerTreeDataProvider("vnoc.structure", sidebar),
    // the tree follows the code by itself; Refresh is the way out if it ever does not: the core reads everything again
    vscode.commands.registerCommand("vnoc.refreshStructure", () => void ready.then(() => client?.sendNotification("vnoc/rebuild"))),
    watchFiles(),
    docs.watchEditors(),
    // these start from a sidebar item (a tree node), or from the editor's menu (the node at the cursor)
    vscode.commands.registerCommand("vnoc.focus", command(async (arg) => onCanvas("focus", (await nodeFor(arg)).id))),
    vscode.commands.registerCommand("vnoc.showOnCanvas", command(async (arg) => onCanvas("reveal", (await nodeFor(arg)).id))),
    vscode.commands.registerCommand("vnoc.showDocs", command(showDocsBeside)),
    vscode.commands.registerCommand("vnoc.addDoc", command(async (arg) => {
      const node = await nodeFor(arg);
      if (node.kind !== "function" && node.kind !== "class") throw new Error("Docs go on functions and classes for now: put the cursor inside one.");
      await addDocTo(node.id, node.docs);
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

type Slice = { path: string; range: { start: { line: number; character: number }; end: { line: number; character: number } } };

/** The text of a definition, from VS Code's document (so unsaved edits show). The path comes from the webview, so it must stay inside the opened folder. */
async function readSlice({ path: relative, range: r }: Slice): Promise<string> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) throw new Error("Open a folder first.");
  const file = path.resolve(folder.uri.fsPath, relative);
  if (!file.startsWith(folder.uri.fsPath + path.sep)) throw new Error(`${relative} is outside the opened folder`);
  const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
  return doc.getText(new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character));
}

/** What a webview may ask of the extension. */
async function answer(method: string, params: any): Promise<unknown> {
  switch (method) {
    case "structure":
      return loadStructure();
    case "addDoc":
      return { added: await addDocTo(params.key, params.docs) };
    case "docs":
      return request("vnoc/docs", params);
    case "docText":
      return docs.text(params.path);
    case "openDoc":
      return void (await docs.open(params.path));
    case "openLink":
      return void (await docs.openLink(params.href, params.from));
    case "code":
      return readSlice(params);
    case "pending": {
      const pending = pendingOnCanvas;
      pendingOnCanvas = undefined;
      return pending ?? null;
    }
    case "docsTarget":
      return docsTarget ?? null;
    case "ping":
      return request("vnoc/ping");
    default:
      throw new Error(`unknown request ${method}`);
  }
}

export async function deactivate() {
  await client?.stop();
}
