import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { CloseAction, type ErrorHandler, LanguageClient, State, TransportKind } from "vscode-languageclient/node";

import { chooseBinary, chooseDrivers } from "@vnoc/shared/binaryChoice.ts";
import { languageExtensions } from "@vnoc/shared/languages.ts";
import type { LanguageInfo, MissingNode, StructureResult, TreeNode } from "@vnoc/shared/messages";
import type { TourCoreNotifications } from "@vnoc/shared/tours.ts";

// The core is a separate process: this extension starts it (vscode-languageclient is only a very good JSON-RPC-over-stdio client here, we use
// its custom-request API and none of the language features). The extension never starts a driver: the core owns the drivers.

/** Saves arrive in bursts (a format-on-save, a `git checkout`): collect the touched files for this long, then send one message. */
const CHANGE_DEBOUNCE_MS = 200;

/**
 * A binary path: the setting if set; from source (F5 or the window suite) the build folder of this repository (`extension/` sits next to `target/` and
 * `drivers/`) and nothing else; installed, `<extension>/bin/`. From source it never falls back to `bin/`: that may hold a copy from an old packaging run,
 * and running it fails with a message about the wrong thing. In Restricted Mode VS Code hands us the default instead of a workspace value
 * (`restrictedConfigurations` in package.json), so a cloned repo cannot choose which program we run.
 */
function binary(ctx: vscode.ExtensionContext, setting: string, name: string, devPath: string[], label: string, buildHint: string): string {
  const exe = process.platform === "win32" ? `${name}.exe` : name;
  const chosen = chooseBinary({
    configured: vscode.workspace.getConfiguration("vnoc").get<string>(setting),
    // the window test suite runs in Test mode: it must use the build folder too, or it would test a stale `bin/`
    developing: ctx.extensionMode !== vscode.ExtensionMode.Production,
    fromSource: path.join(ctx.extensionPath, "..", ...devPath.slice(0, -1), exe),
    packaged: path.join(ctx.extensionPath, "bin", exe),
    exists: fs.existsSync,
    buildHint,
    label,
  });
  if ("error" in chosen) throw new Error(chosen.error);
  return chosen.path;
}

/**
 * The driver paths: `vnoc.drivers`, else `vnoc.driver.path` (deprecated: a list of one); else, from source, every driver built in `drivers/<language>/target/debug`
 * and, installed, every driver in `bin/`. The same restricted-mode rule as `binary()`: the settings are the default's in an untrusted workspace.
 */
function driverPaths(ctx: vscode.ExtensionContext): string[] {
  const settings = vscode.workspace.getConfiguration("vnoc");
  const listed = (settings.get<string[]>("drivers") ?? []).filter((p) => p);
  const single = settings.get<string>("driver.path");
  const chosen = chooseDrivers({
    configured: listed.length > 0 ? listed : single ? [single] : [],
    developing: ctx.extensionMode !== vscode.ExtensionMode.Production,
    driversDir: path.join(ctx.extensionPath, "..", "drivers"),
    binDir: path.join(ctx.extensionPath, "bin"),
    windows: process.platform === "win32",
    list: (dir) => {
      try {
        return fs.readdirSync(dir);
      } catch {
        return [];
      }
    },
  });
  if ("error" in chosen) throw new Error(chosen.error);
  return chosen.paths;
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
  /** The core and the drivers this extension runs (`binary()`'s and `driverPaths()`'s choice), once `ready`: the command line is made from the same files (07 §3). */
  binaryPath: string | undefined;
  driverPaths: string[] = [];

  /** What the core said about its drivers (`vnoc/languages`), after it was ready and again after each restart; empty until then. */
  languages: LanguageInfo[] = [];
  private readonly languagesEmitter = new vscode.EventEmitter<LanguageInfo[]>();
  /** Fires when the core has told which languages it reads: the menus and the watcher are made from it. */
  readonly onLanguages = this.languagesEmitter.event;

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

  private readonly toursChangedEmitter = new vscode.EventEmitter<TourCoreNotifications["vnoc/toursChanged"]>();
  /** Fires when the core says a tour's file changed, appeared or went (`vnoc/toursChanged`): what is shown of tours is asked for again. It is not the index changing. */
  readonly onToursChanged = this.toursChangedEmitter.event;
  private readonly tourPlaceEmitter = new vscode.EventEmitter<TourCoreNotifications["vnoc/tourPlaceChanged"]>();
  /** Fires when another process moved a tour's place (`vnoc/tourPlaceChanged`, not sent for this window's own `vnoc/tourGo`). */
  readonly onTourPlaceChanged = this.tourPlaceEmitter.event;

  /** Whether the user was told, in this run, that the core is not coming back (03 §5): once, not on every retry. */
  private announced = false;

  constructor(
    ctx: vscode.ExtensionContext,
    private readonly output: vscode.OutputChannel,
    /** the core has a new index: ask for the structure again */
    onIndexChanged: (revision: number) => void,
    /** the core is not running and will not start by itself again: it could not start, or the client gave up restarting it. `canRestart`: there is a client to restart. */
    private readonly onStopped: (reason: string, canRestart: boolean) => void,
  ) {
    this.ready = this.start(ctx, onIndexChanged);
    this.ready.then(
      () => void this.loadLanguages(),
      (e) => {
        output.appendLine(String(e));
        this.announceStop(String(e.message ?? e));
      },
    );
  }

  /** Asks the core which languages it reads (it answers once the first build has started every driver), and says them in the log, one line each. */
  private async loadLanguages(): Promise<void> {
    try {
      this.languages = await this.request<LanguageInfo[]>("vnoc/languages");
    } catch (e) {
      this.output.appendLine(`could not ask which languages are read: ${String(e)}`);
      return;
    }
    for (const l of this.languages) {
      const reason = (l.error ?? "").split("\n")[0];
      this.output.appendLine(l.state === "running" ? `${l.name} ${l.version}: ${l.extensions.map((e) => `.${e}`).join(" ")}` : `${l.name}: failed: ${reason}`);
    }
    this.languagesEmitter.fire(this.languages);
  }

  private announceStop(reason: string): void {
    if (this.announced) return;
    this.announced = true;
    this.onStopped(reason, this.client !== undefined);
  }

  /** The core's own version (from `initialize`), or undefined when it is not running. A bug report carries it: the extension and the core can differ. */
  version(): string | undefined {
    return this.client?.state === State.Running ? this.client.initializeResult?.serverInfo?.version : undefined;
  }

  private async start(ctx: vscode.ExtensionContext, onIndexChanged: (revision: number) => void): Promise<void> {
    const folder = vscode.workspace.workspaceFolders?.[0];
    const core = binary(ctx, "server.path", "vnoc", ["target", "debug", "vnoc"], "The V-NOC core", "build it: `cargo build -p vnoc`");
    const drivers = driverPaths(ctx).map((p) => path.resolve(p));
    this.binaryPath = path.resolve(core);
    this.driverPaths = drivers;
    // the client's own handler decides when to restart; when it gives up, the user hears it once, from us, with buttons (its own message is only logged)
    let defaults: ErrorHandler | undefined;
    const errorHandler: ErrorHandler = {
      error: (error, message, count) => defaults!.error(error, message, count),
      closed: async () => {
        const result = await defaults!.closed();
        if (result.action !== CloseAction.DoNotRestart) return result;
        this.announceStop(result.message ?? "the core closed the connection.");
        return { ...result, handled: true };
      },
    };
    this.client = new LanguageClient(
      "vnoc",
      "V-NOC core",
      { command: core, transport: TransportKind.stdio },
      // not a language server: no document selector. The drivers' paths and the opened folder travel to the core in `initializationOptions`.
      { documentSelector: [], outputChannel: this.output, errorHandler, initializationOptions: { drivers, root: folder?.uri.fsPath } },
    );
    defaults = this.client.createDefaultErrorHandler();
    this.client.onDidChangeState(({ newState }) => {
      if (newState !== State.Running) return;
      this.announced = false;
      this.epoch++;
      if (this.epoch > 1) {
        this.restartedEmitter.fire();
        void this.loadLanguages();
      }
    });
    this.client.onNotification("vnoc/indexChanged", (p: { revision: number }) => onIndexChanged(p.revision));
    this.client.onNotification("vnoc/toursChanged", (p: TourCoreNotifications["vnoc/toursChanged"]) => this.toursChangedEmitter.fire(p));
    this.client.onNotification("vnoc/tourPlaceChanged", (p: TourCoreNotifications["vnoc/tourPlaceChanged"]) => this.tourPlaceEmitter.fire(p));
    await this.client.start();
    this.output.appendLine(`core started: ${core}\ndrivers: ${drivers.join(path.delimiter)}`);
  }

  /** `token`: cancelling it sends `$/cancelRequest` for this request, and the core stops the work under it. */
  async request<T>(method: string, params?: object, token?: vscode.CancellationToken): Promise<T> {
    await this.ready;
    // no params is no params: `sendRequest(method, undefined)` goes out as `[null]`, which the core cannot read as "no parameters" (`vnoc/tours`, `vnoc/tourCounts`)
    if (params === undefined) return token ? this.client!.sendRequest<T>(method, token) : this.client!.sendRequest<T>(method);
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

/** The watcher's glob: the extensions of the languages the core reads (with the dot), and the files that change which files are in scope. */
export function watchGlob(extensions: string[]): string {
  return `**/{${[...extensions.map((e) => `*${e}`), ".gitignore", ".vnoc-id", ".vnoc/**"].join(",")}}`;
}

/**
 * Tells the core which files were touched. The glob comes from `vnoc/languages` (`watchGlob`), made again when the list changes; until the core has said, only
 * the files below are watched. `.gitignore` is watched too: it changes which files are in scope. `.vnoc/**` and `.vnoc-id` are V-NOC's
 * own files: a doc added by a teammate's `git pull` shows up like one added here. `onTouched` hears every path at once (the doc panel wants `.md` changes).
 *
 * A folder that appears or goes is reported too (a path with no extension: a deleted one cannot be asked). The extension pattern misses a folder moved
 * with `git mv` (one event, for the folder), and on Linux a file written into a folder in the first moments after it was made. The core walks the whole
 * project on any report, so the folder's own event is enough to find what is inside.
 */
export function watchFiles(core: Core, onTouched: (fsPath: string) => void): vscode.Disposable {
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
  let glob: string | undefined;
  let files: vscode.Disposable | undefined;
  const watch = (extensions: string[]) => {
    const next = watchGlob(extensions);
    if (next === glob) return;
    glob = next;
    files?.dispose();
    const watcher = vscode.workspace.createFileSystemWatcher(next);
    files = vscode.Disposable.from(watcher, watcher.onDidCreate(touch), watcher.onDidChange(touch), watcher.onDidDelete(touch));
  };
  watch(languageExtensions(core.languages).all);
  return vscode.Disposable.from(
    core.onLanguages((languages) => watch(languageExtensions(languages).all)),
    { dispose: () => files?.dispose() },
    folders,
    folders.onDidCreate(folder),
    folders.onDidDelete(folder),
    { dispose: () => clearTimeout(timer) },
  );
}
