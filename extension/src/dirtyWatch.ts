import * as vscode from "vscode";

import { dirtyPaths, nextDirtyFiles, type OpenDocument } from "./dirtyFiles";

/**
 * The set of files with unsaved changes, kept up to date and announced when it changes (the `dirtyFiles` event; a page that starts later asks for it).
 * `isDirty` flips on a keystroke, on a save, on an undo back to the saved text, and on a revert; closing a tab without saving drops the buffer.
 */
export class DirtyFiles {
  private held: string[] = [];

  constructor(private readonly changed: (paths: string[]) => void) {}

  get paths(): string[] {
    return this.held;
  }

  watch(): vscode.Disposable {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (folder) this.held = dirtyPaths(this.documents(), folder.uri.fsPath); // what is dirty already: nobody is told, a page asks when it starts
    return vscode.Disposable.from(
      vscode.workspace.onDidChangeTextDocument(() => this.update()),
      vscode.workspace.onDidSaveTextDocument(() => this.update()),
      // the closed document may still be in `textDocuments` while its event runs, and closing without saving reverts it: leave it out
      vscode.workspace.onDidCloseTextDocument((closed) => this.update(closed)),
    );
  }

  private documents(closing?: vscode.TextDocument): OpenDocument[] {
    return vscode.workspace.textDocuments
      .filter((d) => d !== closing)
      .map((d) => ({ fsPath: d.uri.fsPath, isDirty: d.isDirty, isFile: d.uri.scheme === "file" && !d.isUntitled }));
  }

  private update(closing?: vscode.TextDocument): void {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) return;
    const next = nextDirtyFiles(this.held, this.documents(closing), folder.uri.fsPath);
    if (!next) return;
    this.held = next;
    this.changed(next);
  }
}
