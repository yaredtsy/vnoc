import * as path from "path";

// Which open files have unsaved changes (docs/17-edit-in-canvas.md step 9.1). Pure, so the table is tested; `dirtyWatch.ts` feeds it VS Code's documents.

/** What the set needs of a VS Code document. `isFile`: a file on disk (not untitled, not an output channel or a diff side). */
export interface OpenDocument {
  fsPath: string;
  isDirty: boolean;
  isFile: boolean;
}

/** The paths, relative to the opened folder (with `/`) and sorted, of the documents under it that have unsaved changes. */
export function dirtyPaths(documents: readonly OpenDocument[], folder: string): string[] {
  const found = new Set<string>();
  for (const d of documents) {
    if (!d.isFile || !d.isDirty || !d.fsPath.startsWith(folder + path.sep)) continue;
    found.add(path.relative(folder, d.fsPath).split(path.sep).join("/"));
  }
  return [...found].sort();
}

/** The new set, or null when it is the one `previous` already is (a keystroke in a tab that was dirty changes nothing the pages need). */
export function nextDirtyFiles(previous: readonly string[], documents: readonly OpenDocument[], folder: string): string[] | null {
  const next = dirtyPaths(documents, folder);
  return next.length === previous.length && next.every((p, i) => p === previous[i]) ? null : next;
}
