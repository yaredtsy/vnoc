import * as vscode from "vscode";

import type { Surfaces } from "./surfaces";

// The STRUCTURE view of the side bar (docs/16-ui-redesign/06-sidebar.md §10): a webview view with the same id the native tree had, so its place in the
// container is kept. The page is `webview/src/sidebar/`; what it asks for goes through the same relay as the canvas's.

export const STRUCTURE = "vnoc.structure";

/** `retainContextWhenHidden`: the page is small, and keeping it saves a reload each time the side bar is hidden. */
export function registerSidebar(surfaces: Surfaces): vscode.Disposable {
  return vscode.window.registerWebviewViewProvider(STRUCTURE, { resolveWebviewView: (view) => surfaces.adoptSidebar(view) }, { webviewOptions: { retainContextWhenHidden: true } });
}
