import * as vscode from "vscode";

// The node the text cursor is in gets a mark in the sidebar, not the selection (docs/10-ui-rework/02-sidebar.md §7): the selection drives the doc panel
// and the menus, and it must not jump while you type. Class and function rows have a `resourceUri` in V-NOC's own scheme; this provider decorates the one
// that is the cursor node. (File and folder rows carry their real file URI, which a provider of ours must not decorate: the Explorer shares it. Their mark
// is in the row's description, see structureView.ts.)

export const SCHEME = "vnoc-node";

export const nodeUri = (id: string) => vscode.Uri.from({ scheme: SCHEME, path: `/${id}` });

export class CursorMark implements vscode.FileDecorationProvider {
  private readonly changed = new vscode.EventEmitter<vscode.Uri[]>();
  readonly onDidChangeFileDecorations = this.changed.event;
  private cursor: string | null = null;

  /** The cursor moved: both rows are drawn again. */
  update(next: string | null): void {
    if (next === this.cursor) return;
    const changed = [this.cursor, next].filter((id): id is string => id !== null).map(nodeUri);
    this.cursor = next;
    this.changed.fire(changed);
  }

  provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
    if (uri.scheme !== SCHEME || this.cursor === null || uri.path !== `/${this.cursor}`) return undefined;
    return { badge: "●", tooltip: "the cursor is here", color: new vscode.ThemeColor("editorCursor.foreground") };
  }
}
