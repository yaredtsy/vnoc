import * as vscode from "vscode";

// The two webview tabs: the canvas, and the docs tab beside a code editor. Same bundle and same relay; the mode tells the page which one it is.
// A webview cannot reach the core or any file: it posts a request, `answer` serves it, and the result is posted back.

export type Mode = "canvas" | "docs";

export const CANVAS = "vnoc.canvas";
export const DOCS = "vnoc.docs";

/** What a webview can say besides a request. */
export interface Hooks {
  /** the canvas drew this many nodes (the extension's test suite uses it) */
  rendered(visible: number, hidden: number): void;
  /** the canvas changed its focus stack (focus, back, a crumb): the sidebar follows */
  focusChanged(stack: string[]): void;
  /** the pin in the docs tab: stop (or resume) following the cursor */
  pinned(pinned: boolean): void;
}

type Message = { type: string; id?: number | string; stack?: string[]; method?: string; params?: unknown; visible?: number; hidden?: number; pinned?: boolean };

export class Panels {
  private canvasPanel: vscode.WebviewPanel | undefined;
  private docsPanel: vscode.WebviewPanel | undefined;

  constructor(
    private readonly ctx: vscode.ExtensionContext,
    private readonly answer: (method: string, params: any) => Promise<unknown>,
    private readonly hooks: Hooks,
  ) {}

  get canvas(): vscode.WebviewPanel | undefined {
    return this.canvasPanel;
  }

  get docs(): vscode.WebviewPanel | undefined {
    return this.docsPanel;
  }

  /** Posts to every open V-NOC tab. */
  post(message: object): void {
    this.canvasPanel?.webview.postMessage(message);
    this.docsPanel?.webview.postMessage(message);
  }

  /** The one canvas (a second `Open canvas` or a Focus click shows this one again). */
  showCanvas(): vscode.WebviewPanel {
    if (this.canvasPanel) {
      this.canvasPanel.reveal();
      return this.canvasPanel;
    }
    return this.adopt("canvas", vscode.window.createWebviewPanel(CANVAS, "V-NOC canvas", vscode.ViewColumn.Active, this.options()));
  }

  /** The one docs tab, in the column beside the editor, with the cursor left in the code. */
  showDocs(): vscode.WebviewPanel {
    if (this.docsPanel) {
      this.docsPanel.reveal(undefined, true);
      return this.docsPanel;
    }
    return this.adopt("docs", vscode.window.createWebviewPanel(DOCS, "V-NOC docs", { viewColumn: vscode.ViewColumn.Beside, preserveFocus: true }, this.options()));
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

  private adopt(mode: Mode, panel: vscode.WebviewPanel): vscode.WebviewPanel {
    if (mode === "canvas") this.canvasPanel = panel;
    else this.docsPanel = panel;
    panel.onDidDispose(() => {
      if (this.canvasPanel === panel) this.canvasPanel = undefined;
      if (this.docsPanel === panel) this.docsPanel = undefined;
    });
    panel.webview.html = this.html(panel.webview, mode);
    panel.webview.onDidReceiveMessage((m: Message) => this.receive(panel, m));
    return panel;
  }

  private html(webview: vscode.Webview, mode: Mode): string {
    const asUri = (file: string) => webview.asWebviewUri(vscode.Uri.joinPath(this.dist(), file));
    const nonce = Math.random().toString(36).slice(2) + Math.random().toString(36).slice(2);
    // CSP: no remote anything. Scripts only with our nonce, styles and fonts only from the extension.
    return `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-${nonce}'; style-src ${webview.cspSource} 'unsafe-inline'; img-src ${webview.cspSource} data:; font-src ${webview.cspSource};">
<link rel="stylesheet" href="${asUri("assets/index.css")}">
</head><body><div id="root" data-mode="${mode}"></div>
<script nonce="${nonce}" type="module" src="${asUri("assets/index.js")}"></script></body></html>`;
  }

  private async receive(panel: vscode.WebviewPanel, m: Message): Promise<void> {
    if (m.type === "rendered") return this.hooks.rendered(m.visible ?? 0, m.hidden ?? 0);
    if (m.type === "focusChanged" && Array.isArray(m.stack)) return this.hooks.focusChanged(m.stack);
    if (m.type === "pin") return this.hooks.pinned(m.pinned === true);
    if (m.type === "close") return panel.dispose();
    if (m.type !== "req" || m.method === undefined) return;
    try {
      const result = await this.answer(m.method, m.params);
      panel.webview.postMessage({ type: "res", id: m.id, result });
    } catch (e) {
      panel.webview.postMessage({ type: "res", id: m.id, error: String((e as Error).message ?? e) });
    }
  }
}
