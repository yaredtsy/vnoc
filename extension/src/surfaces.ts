import * as vscode from "vscode";

import type { Events, Notes, Page, RequestMessage } from "@vnoc/shared/messages";

import { type Handlers, event, serve } from "./relay";

// The three webview surfaces: the canvas and the docs tab beside a code editor (one bundle; the mode tells the page which one it is), and the STRUCTURE
// view in the side bar (its own, smaller bundle: 06 §10). Same relay for all three.
// A webview cannot reach the core or any file: it posts a request, the relay serves it, and the result is posted back.

export type Mode = "canvas" | "docs";

export const CANVAS = "vnoc.canvas";
export const DOCS = "vnoc.docs";

export class Surfaces {
  private canvasPanel: vscode.WebviewPanel | undefined;
  private docsPanel: vscode.WebviewPanel | undefined;
  private sidebarView: vscode.WebviewView | undefined;
  /** The STRUCTURE page has drawn at least once, so it is listening for the events a command sends it. */
  private sidebarDrew = false;
  private sidebarDrawnWaiters: (() => void)[] = [];

  constructor(
    private readonly ctx: vscode.ExtensionContext,
    private readonly handlers: Handlers,
    /** what a webview says that needs no answer (`Notes`); `close` is handled here */
    private readonly onNote: (note: Notes) => void,
    /** the canvas tab or the docs tab was closed (not hidden): a webview panel cannot veto its own close, so the host asks after it (docs/17-edit-in-canvas.md §7, 9.5c, §8) */
    private readonly onPageClosed: (page: Page) => void = () => {},
  ) {}

  get canvas(): vscode.WebviewPanel | undefined {
    return this.canvasPanel;
  }

  get docs(): vscode.WebviewPanel | undefined {
    return this.docsPanel;
  }

  get sidebar(): vscode.WebviewView | undefined {
    return this.sidebarView;
  }

  /** Sends an event to every open V-NOC page: the tabs and the side bar. */
  emit<E extends keyof Events>(name: E, data: Events[E]): void {
    this.canvasPanel?.webview.postMessage(event(name, data));
    this.docsPanel?.webview.postMessage(event(name, data));
    this.sidebarView?.webview.postMessage(event(name, data));
  }

  /**
   * Resolves when the STRUCTURE page has drawn (at once if it has), or after `ms`: a page with no folder open never draws a list, and the command that waits must not hang.
   * An event posted before the page is listening is lost, so a command that needs the page waits for this first.
   */
  whenSidebarDrew(ms: number): Promise<void> {
    if (this.sidebarDrew) return Promise.resolve();
    return new Promise((resolve) => {
      const done = () => (clearTimeout(timer), resolve());
      const timer = setTimeout(() => {
        this.sidebarDrawnWaiters = this.sidebarDrawnWaiters.filter((w) => w !== done);
        resolve();
      }, ms);
      this.sidebarDrawnWaiters.push(done);
    });
  }

  /** The STRUCTURE view was resolved (it is shown for the first time, or VS Code made it again): give it its page. */
  adoptSidebar(view: vscode.WebviewView): void {
    this.sidebarView = view;
    this.sidebarDrew = false; // a new page: it has not drawn, and nothing it is sent is heard until it has
    view.webview.options = this.options();
    view.onDidDispose(() => {
      if (this.sidebarView === view) this.sidebarView = undefined;
    });
    view.webview.html = this.html(view.webview, { script: "assets/sidebar.js", css: "assets/sidebar.css", attributes: `data-mode="sidebar" data-folder="${(vscode.workspace.workspaceFolders?.length ?? 0) > 0}" data-hover-delay="${vscode.workspace.getConfiguration("workbench.hover").get<number>("delay", 500)}"` });
    view.webview.onDidReceiveMessage((m: RequestMessage | Notes) => {
      if (m.type === "sidebarRendered" && this.sidebarView === view) {
        this.sidebarDrew = true;
        this.sidebarDrawnWaiters.splice(0).forEach((w) => w());
      }
      void this.receive(view.webview, m, () => {});
    });
  }

  /**
   * The one canvas (a second `Open canvas` or a Focus click shows this one again). `preserveFocus`: bring it to the front of its group without
   * taking the keyboard away from where the user is (a click in the sidebar).
   */
  showCanvas(opts: { preserveFocus?: boolean; beside?: boolean } = {}): vscode.WebviewPanel {
    const column = opts.beside ? vscode.ViewColumn.Beside : undefined;
    if (this.canvasPanel) {
      this.canvasPanel.reveal(column, opts.preserveFocus);
      return this.canvasPanel;
    }
    return this.adopt("canvas", vscode.window.createWebviewPanel(CANVAS, "V-NOC canvas", { viewColumn: column ?? vscode.ViewColumn.Active, preserveFocus: opts.preserveFocus }, this.options()));
  }

  /**
   * The one docs tab, in the column beside the editor, with the cursor left in the code. It follows the cursor, unless `pinTo` names a node
   * (Show Docs on a row or a card): then it shows that one until the pin is released; `doc` (a doc's path, with `pinTo`) is the doc of it to show. A new tab learns both from its
   * page (no message can be lost).
   */
  showDocs(pinTo?: string, doc?: string): vscode.WebviewPanel {
    if (this.docsPanel) {
      this.docsPanel.reveal(undefined, true);
      if (pinTo) this.docsPanel.webview.postMessage(event("showNode", doc ? { id: pinTo, path: doc } : { id: pinTo }));
      return this.docsPanel;
    }
    return this.adopt("docs", vscode.window.createWebviewPanel(DOCS, "V-NOC docs", { viewColumn: vscode.ViewColumn.Beside, preserveFocus: true }, this.options()), pinTo, doc);
  }

  /** A tab VS Code restored after a restart. */
  restore(mode: Mode, panel: vscode.WebviewPanel): void {
    panel.webview.options = this.options();
    this.adopt(mode, panel);
  }

  private options(): vscode.WebviewPanelOptions & vscode.WebviewOptions {
    return { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [this.dist()] };
  }

  private dist(): vscode.Uri {
    return vscode.Uri.joinPath(this.ctx.extensionUri, "media", "webview");
  }

  private adopt(mode: Mode, panel: vscode.WebviewPanel, pinTo?: string, doc?: string): vscode.WebviewPanel {
    if (mode === "canvas") this.canvasPanel = panel;
    else this.docsPanel = panel;
    // the tab's icon: an SVG file does not take the theme's colour, so there is one for each theme kind
    panel.iconPath = { light: vscode.Uri.joinPath(this.ctx.extensionUri, "media", "tab-light.svg"), dark: vscode.Uri.joinPath(this.ctx.extensionUri, "media", "tab-dark.svg") };
    if (mode === "canvas") void vscode.commands.executeCommand("setContext", "vnoc.canvasOpen", true);
    panel.onDidDispose(() => {
      if (this.canvasPanel === panel) {
        this.canvasPanel = undefined;
        void vscode.commands.executeCommand("setContext", "vnoc.canvasOpen", false);
        this.onPageClosed("canvas");
      }
      if (this.docsPanel === panel) {
        this.docsPanel = undefined;
        this.onPageClosed("docs");
      }
    });
    panel.webview.html = this.html(panel.webview, { script: "assets/index.js", css: "assets/index.css", attributes: `data-mode="${mode}"${pinTo ? ` data-node="${escapeAttribute(pinTo)}"` : ""}${pinTo && doc ? ` data-doc="${escapeAttribute(doc)}"` : ""}` });
    panel.webview.onDidReceiveMessage((m: RequestMessage | Notes) => void this.receive(panel.webview, m, () => panel.dispose()));
    return panel;
  }

  /** The page: `script` and `css` are files of the bundle in `media/webview`; `attributes` go on the root element (what the page needs to know before it asks). */
  private html(webview: vscode.Webview, page: { script: string; css: string; attributes: string }): string {
    const asUri = (file: string) => webview.asWebviewUri(vscode.Uri.joinPath(this.dist(), file));
    const nonce = Math.random().toString(36).slice(2) + Math.random().toString(36).slice(2);
    // CSP: no remote anything. Scripts only with our nonce, styles and fonts only from the extension.
    return `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-${nonce}'; style-src ${webview.cspSource} 'unsafe-inline'; img-src ${webview.cspSource} data:; font-src ${webview.cspSource};">
<link rel="stylesheet" href="${asUri(page.css)}">
</head><body><div id="root" ${page.attributes}></div>
<script nonce="${nonce}" type="module" src="${asUri(page.script)}"></script></body></html>`;
  }

  private async receive(webview: vscode.Webview, message: RequestMessage | Notes, close: () => void): Promise<void> {
    if (message.type === "req") return void (await webview.postMessage(await serve(this.handlers, message)));
    if (message.type === "close") return close();
    this.onNote(message);
  }
}

/** A node key can hold `"`, `<` and `&` (a Python name cannot, a path can): it goes into an attribute. */
function escapeAttribute(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
}
