import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { LanguageClient, TransportKind } from "vscode-languageclient/node";

import { addDoc } from "./addDoc";
import { Docs } from "./docs";
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

const PANEL = "vnoc.canvas";

/** Saves arrive in bursts (a format-on-save, a `git checkout`): collect the touched files for this long, then send one message. */
const CHANGE_DEBOUNCE_MS = 200;

/** The one canvas panel (a second `Open canvas` or a Focus click shows this one again). */
let canvas: vscode.WebviewPanel | undefined;
/** A focus asked for while the canvas was closed: the new webview takes it when it starts. */
let pendingFocus: string | undefined;

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

/** The absolute path of the git root, from the last structure. Node ids and the paths in `vnoc/attach` start there. */
let gitRoot: string | undefined;

/** The structure of the opened folder; the canvas and the sidebar both read it through here. It waits for the core's first build. */
async function loadStructure<T = { root: TreeNode }>(): Promise<T> {
  if (!vscode.workspace.workspaceFolders?.[0]) throw new Error("Open a folder first: V-NOC shows the code of the folder you opened.");
  const structure = await request<T & { git_root: string }>("vnoc/structure");
  gitRoot = structure.git_root;
  return structure;
}

/** The "Add doc" flow for the node with this id (a sidebar item, or a card on the canvas). Resolves to whether a doc was added. */
async function addDocTo(key: string, existingDocs: number): Promise<boolean> {
  const root = gitRoot ?? (await loadStructure<{ git_root: string }>()).git_root;
  return addDoc(request, root, key, existingDocs);
}

let sidebar: StructureView;
const docs = new Docs(() => gitRoot, (message) => canvas?.webview.postMessage(message));

/** The core has a new index: the sidebar redraws, and an open canvas asks for the new structure. */
function indexChanged(revision: number) {
  void sidebar.refresh();
  canvas?.webview.postMessage({ type: "indexChanged", revision });
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

  sidebar = new StructureView(async () => (await loadStructure()).root);
  void sidebar.refresh();

  // Registered synchronously, before anything is awaited: on restart VS Code waits for this serializer (S1).
  ctx.subscriptions.push(
    vscode.commands.registerCommand("vnoc.openCanvas", () => showCanvas(ctx)),
    vscode.window.registerWebviewPanelSerializer(PANEL, {
      async deserializeWebviewPanel(panel) {
        panel.webview.options = panelOptions(ctx);
        adopt(ctx, panel);
      },
    }),
    vscode.window.registerTreeDataProvider("vnoc.structure", sidebar),
    // the tree follows the code by itself; Refresh is the way out if it ever does not: the core reads everything again
    vscode.commands.registerCommand("vnoc.refreshStructure", () => void ready.then(() => client?.sendNotification("vnoc/rebuild"))),
    watchFiles(),
    docs.watchEditors(),
    // the hover button on a sidebar item: the canvas opens (or comes to the front) showing just that item
    vscode.commands.registerCommand("vnoc.focus", (node: TreeNode) => focusOnCanvas(ctx, node.id)),
    vscode.commands.registerCommand("vnoc.addDoc", (node: TreeNode) => addDocTo(node.id, node.docs)),
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

function showCanvas(ctx: vscode.ExtensionContext): vscode.WebviewPanel {
  if (canvas) {
    canvas.reveal();
    return canvas;
  }
  const panel = vscode.window.createWebviewPanel(PANEL, "V-NOC canvas", vscode.ViewColumn.Active, panelOptions(ctx));
  adopt(ctx, panel);
  return panel;
}

function adopt(ctx: vscode.ExtensionContext, panel: vscode.WebviewPanel) {
  canvas = panel;
  panel.onDidDispose(() => {
    if (canvas === panel) canvas = undefined;
  });
  setupPanel(ctx, panel);
}

function focusOnCanvas(ctx: vscode.ExtensionContext, id: string) {
  const wasOpen = canvas !== undefined;
  const panel = showCanvas(ctx);
  // an open canvas is told now; a canvas that is just starting asks for it (the `pendingFocus` request)
  if (wasOpen) panel.webview.postMessage({ type: "focus", id });
  else pendingFocus = id;
}

function distUri(ctx: vscode.ExtensionContext): vscode.Uri {
  return vscode.Uri.joinPath(ctx.extensionUri, "media", "webview");
}

function panelOptions(ctx: vscode.ExtensionContext): vscode.WebviewPanelOptions & vscode.WebviewOptions {
  return { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [distUri(ctx)] };
}

function setupPanel(ctx: vscode.ExtensionContext, panel: vscode.WebviewPanel) {
  const dist = distUri(ctx);
  const asUri = (f: string) => panel.webview.asWebviewUri(vscode.Uri.joinPath(dist, f));
  const nonce = Math.random().toString(36).slice(2) + Math.random().toString(36).slice(2);
  // CSP: no remote anything. Scripts only with our nonce, styles and fonts only from the extension.
  panel.webview.html = `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-${nonce}'; style-src ${panel.webview.cspSource} 'unsafe-inline'; img-src ${panel.webview.cspSource} data:; font-src ${panel.webview.cspSource};">
<link rel="stylesheet" href="${asUri("assets/index.css")}">
</head><body><div id="root"></div>
<script nonce="${nonce}" type="module" src="${asUri("assets/index.js")}"></script></body></html>`;

  // The webview cannot reach the core. It posts a request; we relay it and post the answer back.
  panel.webview.onDidReceiveMessage(async (m: { type: string; id: number; method: string; params?: any; visible?: number; hidden?: number }) => {
    if (m.type === "rendered") {
      rendered = { visible: m.visible ?? 0, hidden: m.hidden ?? 0 };
      output.appendLine(`webview drew ${rendered.visible} nodes (${rendered.hidden} hidden)`);
      renderWaiters.splice(0).forEach((w) => w(rendered!));
      return;
    }
    if (m.type !== "req") return;
    try {
      let result: unknown;
      if (m.method === "structure") {
        result = await loadStructure();
      } else if (m.method === "addDoc") {
        result = { added: await addDocTo(m.params.key, m.params.docs) };
      } else if (m.method === "docs") {
        result = await request("vnoc/docs", m.params);
      } else if (m.method === "docText") {
        result = await docs.text(m.params.path);
      } else if (m.method === "openDoc") {
        await docs.open(m.params.path);
        result = null;
      } else if (m.method === "openLink") {
        await docs.openLink(m.params.href, m.params.from);
        result = null;
      } else if (m.method === "code") {
        result = await readSlice(m.params);
      } else if (m.method === "pendingFocus") {
        result = pendingFocus ?? null;
        pendingFocus = undefined;
      } else if (m.method === "ping") {
        result = await request("vnoc/ping");
      } else {
        throw new Error(`unknown request ${m.method}`);
      }
      panel.webview.postMessage({ type: "res", id: m.id, result });
    } catch (e) {
      panel.webview.postMessage({ type: "res", id: m.id, error: String((e as Error).message ?? e) });
    }
  });
}

export async function deactivate() {
  await client?.stop();
}
