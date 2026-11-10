import * as vscode from "vscode";

import type { Events, Notes, RequestMessage } from "@vnoc/shared/messages";

import { type Handlers, event, serve } from "./relay";

// The two webview tabs: the canvas, and the docs tab beside a code editor. Same bundle and same relay; the mode tells the page which one it is.
// A webview cannot reach the core or any file: it posts a request, the relay serves it, and the result is posted back.

export type Mode = "canvas" | "docs";

export const CANVAS = "vnoc.canvas";
export const DOCS = "vnoc.docs";

export class Panels {
  private canvasPanel: vscode.WebviewPanel | undefined;
  private docsPanel: vscode.WebviewPanel | undefined;

  constructor(
    private readonly ctx: vscode.ExtensionContext,
    private readonly handlers: Handlers,
    /** what a webview says that needs no answer (`Notes`); `close` is handled here */
    private readonly onNote: (note: Notes) => void,
  ) {}

  get canvas(): vscode.WebviewPanel | undefined {
    return this.canvasPanel;
  }

  get docs(): vscode.WebviewPanel | undefined {
    return this.docsPanel;
  }

  /** Sends an event to every open V-NOC tab. */
  emit<E extends keyof Events>(name: E, data: Events[E]): void {
    this.canvasPanel?.webview.postMessage(event(name, data));
    this.docsPanel?.webview.postMessage(event(name, data));
  }

  /** The one canvas (a second `Open canvas` or a Focus click shows this one again). */
  showCanvas(): vscode.WebviewPanel {
    if (this.canvasPanel) {
      this.canvasPanel.reveal();
      return this.canvasPanel;
    }
    return this.adopt("canvas", vscode.window.createWebviewPanel(CANVAS, "V-NOC canvas", vscode.ViewColumn.Active, this.options()));
  }

  /**
   * The one docs tab, in the column beside the editor, with the cursor left in the code. It follows the cursor, unless `pinTo` names a node
   * (Show Docs on a row or a card): then it shows that one until the pin is released. A new tab learns it from its page (no message can be lost).
   */
  showDocs(pinTo?: string): vscode.WebviewPanel {
    if (this.docsPanel) {
      this.docsPanel.reveal(undefined, true);
      if (pinTo) this.docsPanel.webview.postMessage(event("showNode", { id: pinTo }));
      return this.docsPanel;
    }
    return this.adopt("docs", vscode.window.createWebviewPanel(DOCS, "V-NOC docs", { viewColumn: vscode.ViewColumn.Beside, preserveFocus: true }, this.options()), pinTo);
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

  private adopt(mode: Mode, panel: vscode.WebviewPanel, pinTo?: string): vscode.WebviewPanel {
    if (mode === "canvas") this.canvasPanel = panel;
    else this.docsPanel = panel;
    panel.onDidDispose(() => {
      if (this.canvasPanel === panel) this.canvasPanel = undefined;
      if (this.docsPanel === panel) this.docsPanel = undefined;
    });
    panel.webview.html = this.html(panel.webview, mode, pinTo);
    panel.webview.onDidReceiveMessage((m: RequestMessage | Notes) => this.receive(panel, m));
    return panel;
  }

  private html(webview: vscode.Webview, mode: Mode, pinTo?: string): string {
    const asUri = (file: string) => webview.asWebviewUri(vscode.Uri.joinPath(this.dist(), file));
    const nonce = Math.random().toString(36).slice(2) + Math.random().toString(36).slice(2);
    // CSP: no remote anything. Scripts only with our nonce, styles and fonts only from the extension.
    return `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-${nonce}'; style-src ${webview.cspSource} 'unsafe-inline'; img-src ${webview.cspSource} data:; font-src ${webview.cspSource};">
<link rel="stylesheet" href="${asUri("assets/index.css")}">
</head><body><div id="root" data-mode="${mode}"${pinTo ? ` data-node="${escapeAttribute(pinTo)}"` : ""}></div>
<script nonce="${nonce}" type="module" src="${asUri("assets/index.js")}"></script></body></html>`;
  }

  private async receive(panel: vscode.WebviewPanel, message: RequestMessage | Notes): Promise<void> {
    if (message.type === "req") return void (await panel.webview.postMessage(await serve(this.handlers, message)));
    if (message.type === "close") return panel.dispose();
    this.onNote(message);
  }
}

/** A node key can hold `"`, `<` and `&` (a Python name cannot, a path can): it goes into an attribute. */
function escapeAttribute(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
}
