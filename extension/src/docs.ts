import * as path from "path";
import * as vscode from "vscode";

import type { Events } from "@vnoc/shared/messages";

// What the doc panel needs from the editor: the text of a doc (from VS Code's own copy, so unsaved typing shows), opening a doc or a
// link, and a push of the new text when a doc changes. The panel never reads files and never writes; every path it sends is checked here.

/** Typing is sent to the panel at most this often. */
const PUSH_DEBOUNCE_MS = 150;

export class Docs {
  private readonly timers = new Map<string, NodeJS.Timeout>();

  /** `gitRoot` is where `.vnoc/` is; `emit` sends an event to every open V-NOC tab. */
  constructor(
    private readonly gitRoot: () => string | undefined,
    private readonly emit: <E extends keyof Events>(name: E, data: Events[E]) => void,
  ) {}

  /** A doc is a Markdown file inside `<git root>/.vnoc/nodes/`. The webview names files, so nothing else is ever read for it. */
  private check(file: string): string {
    const nodes = path.join(this.gitRoot() ?? "\0", ".vnoc", "nodes") + path.sep;
    const resolved = path.resolve(file);
    if (!resolved.startsWith(nodes) || !resolved.endsWith(".md")) throw new Error(`${file} is not a V-NOC doc`);
    return resolved;
  }

  async text(file: string): Promise<string> {
    return (await vscode.workspace.openTextDocument(vscode.Uri.file(this.check(file)))).getText();
  }

  /** "Edit": a normal editor tab beside the canvas. The panel is its live preview. */
  async open(file: string): Promise<void> {
    await vscode.window.showTextDocument(vscode.Uri.file(this.check(file)), { viewColumn: vscode.ViewColumn.Beside });
  }

  /** A link in a doc: `http(s)` opens in the browser, a relative link opens that file (inside the repository). */
  async openLink(href: string, from: string): Promise<void> {
    if (/^https?:\/\//i.test(href)) {
      await vscode.env.openExternal(vscode.Uri.parse(href));
      return;
    }
    const root = this.gitRoot();
    const target = path.resolve(path.dirname(this.check(from)), decodeURIComponent(href.split("#")[0]));
    if (!root || !target.startsWith(root + path.sep)) throw new Error(`${href} points outside the repository`);
    await vscode.commands.executeCommand("vscode.open", vscode.Uri.file(target));
  }

  /** Sends the new text of a doc to the panel (collected for a moment: typing makes a burst of changes). */
  changed(file: string): void {
    let doc: string;
    try {
      doc = this.check(file);
    } catch {
      return; // not a V-NOC doc
    }
    clearTimeout(this.timers.get(doc));
    this.timers.set(
      doc,
      setTimeout(() => {
        this.timers.delete(doc);
        this.text(doc).then((text) => this.emit("docText", { path: doc, text }), () => {});
      }, PUSH_DEBOUNCE_MS),
    );
  }

  /** Typing in an editor tab updates the panel. (A change on disk comes through the file watcher, which calls `changed`.) */
  watchEditors(): vscode.Disposable {
    return vscode.Disposable.from(vscode.workspace.onDidChangeTextDocument((e) => this.changed(e.document.uri.fsPath)), { dispose: () => this.timers.forEach(clearTimeout) });
  }
}
