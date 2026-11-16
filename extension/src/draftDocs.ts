import * as vscode from "vscode";

// The drafts' side of "Show Diff" (docs/17-edit-in-canvas.md §4): a document that is not on disk, served from memory, so that VS Code's diff editor can put it beside
// the file. Each diff gets its own address; the text of the last few is kept.

export const DRAFT_SCHEME = "vnoc-draft";
const KEEP = 20;

export class DraftDocs implements vscode.TextDocumentContentProvider {
  private readonly texts = new Map<string, string>();
  private next = 0;

  provideTextDocumentContent(uri: vscode.Uri): string {
    return this.texts.get(uri.toString()) ?? "";
  }

  /** An address for `text`, named like `file` (so the editor colours it as the file is) and different on each call. */
  add(file: vscode.Uri, text: string): vscode.Uri {
    const uri = file.with({ scheme: DRAFT_SCHEME, query: `draft-${++this.next}` });
    this.texts.set(uri.toString(), text);
    for (const old of [...this.texts.keys()].slice(0, Math.max(0, this.texts.size - KEEP))) this.texts.delete(old);
    return uri;
  }
}
