import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { LanguageClient, State, TransportKind } from "vscode-languageclient/node";

import type { MissingNode, StructureResult, TreeNode } from "@vnoc/shared/messages";

// The core is a separate process: this extension starts it (vscode-languageclient is only a very good JSON-RPC-over-stdio client here, we use
// its custom-request API and none of the language features). The extension never starts a driver: the core owns the drivers.

/** Saves arrive in bursts (a format-on-save, a `git checkout`): collect the touched files for this long, then send one message. */
const CHANGE_DEBOUNCE_MS = 200;

/**
 * A binary path: the setting if set; else, when running from source (F5), the build folder of this repository
 * (`extension/` sits next to `target/` and `drivers/`); else `<extension>/bin/` (the installed VSIX).
 * From source (F5 or the window suite) the build folder wins, because `bin/` may hold a stale copy left by an earlier packaging run.
 * In Restricted Mode VS Code hands us the default instead of a workspace value (`restrictedConfigurations` in package.json),
 * so a cloned repo cannot choose which program we run.
 */
function binary(ctx: vscode.ExtensionContext, setting: string, name: string, devPath: string[]): string {
  const configured = vscode.workspace.getConfiguration("vnoc").get<string>(setting);
  if (configured) return configured;
  const exe = process.platform === "win32" ? `${name}.exe` : name;
  const fromSource = path.join(ctx.extensionPath, "..", ...devPath.slice(0, -1), exe);
  const packaged = path.join(ctx.extensionPath, "bin", exe);
  // the window test suite runs in Test mode: it must use the build folder too, or it would test a stale `bin/`
  const developing = ctx.extensionMode !== vscode.ExtensionMode.Production;
  return developing && fs.existsSync(fromSource) ? fromSource : fs.existsSync(packaged) ? packaged : fromSource;
}

/** The running core, and the last structure it sent. */
export class Core {
  private client: LanguageClient | undefined;
  /** Resolves when the core is running. Everything that talks to it awaits this first (S1: never block activate()). */
  readonly ready: Promise<void>;

  /** The last structure the core sent. The editor menus look the node at the cursor up here. */
  tree: TreeNode | undefined;
  /** The Missing nodes of the last structure: "Attach a Missing Doc Here…" lists them. */
  missing: MissingNode[] = [];
  /** The absolute path of the git root. Node ids and the paths in `vnoc/attach` start there. */
  gitRoot: string | undefined;

  /**
   * Which run of the core this is: 1 for the first, one more each time it starts again (a crash, or `restart`). A restarted core counts the index
   * revision from the start again, so a revision from one epoch must never be compared with one from another.
   */
  epoch = 0;
  private readonly restartedEmitter = new vscode.EventEmitter<void>();
  /** Fires when the core is running again after it was running before: everything held that came from the old one is meaningless. */
  readonly onRestarted = this.restartedEmitter.event;

  private readonly structureArrived = new vscode.EventEmitter<StructureResult>();
  /** Fires, before `structure()` returns, every time a structure arrives: what the session points at may be gone. */
  readonly onStructure = this.structureArrived.event;

  constructor(
    ctx: vscode.ExtensionContext,
    private readonly output: vscode.OutputChannel,
    /** the core has a new index: ask for the structure again */
    onIndexChanged: (revision: number) => void,
  ) {
    this.ready = this.start(ctx, onIndexChanged);
    this.ready.catch((e) => {
      output.appendLine(String(e));
      vscode.window.showErrorMessage(String(e.message ?? e));
    });
  }

  private async start(ctx: vscode.ExtensionContext, onIndexChanged: (revision: number) => void): Promise<void> {
    const folder = vscode.workspace.workspaceFolders?.[0];
    const core = binary(ctx, "server.path", "vnoc", ["target", "debug", "vnoc"]);
    const driver = binary(ctx, "driver.path", "vnoc-driver-python", ["drivers", "python", "target", "debug", "vnoc-driver-python"]);
    if (!fs.existsSync(core)) {
      throw new Error(`The V-NOC core is not at ${core}. Build it: cargo build -p vnoc`);
    }
    this.client = new LanguageClient(
      "vnoc",
      "V-NOC core",
      { command: core, transport: TransportKind.stdio },
      // not a language server: no document selector. The driver's path and the opened folder travel to the core in `initializationOptions`.
      { documentSelector: [], outputChannel: this.output, initializationOptions: { driverPath: driver, root: folder?.uri.fsPath } },
    );
    this.client.onDidChangeState(({ newState }) => {
      if (newState !== State.Running) return;
      this.epoch++;
      if (this.epoch > 1) this.restartedEmitter.fire();
    });
    this.client.onNotification("vnoc/indexChanged", (p: { revision: number }) => onIndexChanged(p.revision));
    await this.client.start();
    this.output.appendLine(`core started: ${core}\ndriver: ${driver}`);
  }

  /** `token`: cancelling it sends `$/cancelRequest` for this request, and the core stops the work under it. */
  async request<T>(method: string, params?: object, token?: vscode.CancellationToken): Promise<T> {
    await this.ready;
    return token ? this.client!.sendRequest<T>(method, params, token) : this.client!.sendRequest<T>(method, params);
  }

  async notify(method: string, params?: object): Promise<void> {
    await this.ready;
    await this.client?.sendNotification(method, params);
  }

  /** The structure of the opened folder; the canvas, the docs tab and the sidebar all read it through here. It waits for the core's first build. */
  async loadStructure(): Promise<StructureResult> {
    if (!vscode.workspace.workspaceFolders?.[0]) throw new Error("Open a folder first: V-NOC shows the code of the folder you opened.");
    const structure = await this.request<StructureResult>("vnoc/structure");
    this.tree = structure.root;
    this.gitRoot = structure.git_root;
    this.missing = structure.missing;
    this.structureArrived.fire(structure);
    return structure;
  }

  /** Shows the V-NOC output channel (the "Show Log" button of the sidebar). */
  showLog(): void {
    this.output.show();
  }

  /** Stops the core and starts it again (the window suite's way back after it broke the core on purpose). */
  async restart(): Promise<void> {
    await this.client?.restart();
  }

  async stop(): Promise<void> {
    await this.client?.stop();
  }
}

/**
 * Tells the core which files were touched. `.gitignore` is watched too: it changes which files are in scope. `.vnoc/**` and `.vnoc-id` are V-NOC's
 * own files: a doc added by a teammate's `git pull` shows up like one added here. `onTouched` hears every path at once (the doc panel wants `.md` changes).
 *
 * A folder that appears or goes is reported too (a path with no extension: a deleted one cannot be asked). The `.py` pattern misses a folder moved
 * with `git mv` (one event, for the folder), and on Linux a file written into a folder in the first moments after it was made. The core walks the whole
 * project on any report, so the folder's own event is enough to find what is inside.
 */
export function watchFiles(core: Core, onTouched: (fsPath: string) => void): vscode.Disposable {
  const watcher = vscode.workspace.createFileSystemWatcher("**/{*.py,.gitignore,.vnoc-id,.vnoc/**}");
  const folders = vscode.workspace.createFileSystemWatcher("**/*", false, true, false); // created and deleted: a folder's content is not its own change
  const touched = new Set<string>();
  let timer: NodeJS.Timeout | undefined;
  const touch = (uri: vscode.Uri) => {
    onTouched(uri.fsPath);
    report(uri);
  };
  const report = (uri: vscode.Uri) => {
    touched.add(uri.fsPath);
    clearTimeout(timer);
    timer = setTimeout(() => {
      const paths = [...touched];
      touched.clear();
      void core.notify("vnoc/filesChanged", { paths });
    }, CHANGE_DEBOUNCE_MS);
  };
  const folder = (uri: vscode.Uri) => {
    if (path.extname(uri.fsPath) === "" && !/[\\/](\.git|node_modules)[\\/]/.test(uri.fsPath)) report(uri);
  };
  return vscode.Disposable.from(
    watcher,
    watcher.onDidCreate(touch),
    watcher.onDidChange(touch),
    watcher.onDidDelete(touch),
    folders,
    folders.onDidCreate(folder),
    folders.onDidDelete(folder),
    { dispose: () => clearTimeout(timer) },
  );
}
