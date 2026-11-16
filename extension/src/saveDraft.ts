// What Save does with a draft (docs/17-edit-in-canvas.md §6). Pure, so the table is tested; `commands.ts` (`editSaveOf`) only reads the document and calls VS Code.

export interface SaveFacts {
  /** the document's version when the draft started */
  draftVersion: number;
  /** the document's version now */
  version: number;
  /** the text at the draft's range now */
  current: string;
  /** the range's text when the draft started */
  base: string;
}

/**
 * Which row of §6's table applies. The first row: the version is the draft's and the range still holds `base`: write at the draft's own range.
 * Everything else is a conflict for now: the second and third rows (an edit elsewhere in the file moved the function; the function itself changed) need
 * `vnoc/locate`, which is 9.4's. A version that is the draft's with other text under the range cannot happen with a range from `editStart`, and is a conflict too.
 */
export function decideSave(f: SaveFacts): "write" | "conflict" {
  return f.version === f.draftVersion && f.current === f.base ? "write" : "conflict";
}

/**
 * The answer after `applyEdit`: it refused, a conflict (nothing was written); it applied and the document was saved, `saved`; it applied and the document was
 * not saved (it was dirty before, E4: the user's own unsaved work is never saved for them, or the save failed), `written`: in the buffer, not on disk.
 */
export function outcomeAfterApply(applied: boolean, saved: boolean): "saved" | "written" | "conflict" {
  if (!applied) return "conflict";
  return saved ? "saved" : "written";
}
