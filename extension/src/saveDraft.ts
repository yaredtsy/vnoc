// What Save does with a draft (docs/17-edit-in-canvas.md §6). Pure, so the tables are tested; `commands.ts` (`editSaveOf`) only reads the document, asks the core
// where the function is, and calls VS Code. Three questions are asked, in this order, and each is a function here.

/** What the host knows when a draft's Save arrives, before it asks the core anything. */
export interface FirstFacts {
  /** the document's version when the draft started */
  draftVersion: number;
  /** the document's version now */
  version: number;
  /** the text at the draft's own range now */
  current: string;
  /** the range's text when the draft started */
  base: string;
  /** what Save would write: the draft re-indented (`reindent`) */
  written: string;
}

/**
 * Row one of §6, and the way to the others.
 * - `write`: the version is the draft's and the range still holds `base`: write at the draft's own range.
 * - `already`: the version is the draft's and the range holds what Save would write: it was saved already (a second Save, or a Save the page lost with a reload).
 * - `conflict`: the version is the draft's and the range holds something else: never write over it.
 * - `locate`: the version moved on. An edit elsewhere may have moved the function, or changed it: the core says where it is now (`vnoc/locate`).
 */
export function firstStep(f: FirstFacts): "write" | "already" | "conflict" | "locate" {
  if (f.version !== f.draftVersion) return "locate";
  if (f.current === f.base) return "write";
  return f.current === f.written ? "already" : "conflict";
}

export interface LocatedFacts {
  /** the core found the function in the text it was given */
  found: boolean;
  /** the version of the text the core was given */
  sentVersion: number;
  /** the document's version now, read after the core answered */
  version: number;
  /** the text at the range the core gave, read now (`""` when it found nothing) */
  current: string;
  base: string;
  written: string;
}

/**
 * Rows two and three of §6, after `vnoc/locate` answered.
 * - `gone`: the text has no such function.
 * - `conflict`: the document changed while the core was looking (the answer is about text that is not there any more), or the function at the new place is not
 *   the one the draft started from.
 * - `write`: the function is where the core says, and holds `base`: write at the new range.
 * - `already`: the function at the new place holds what Save would write.
 */
export function afterLocate(f: LocatedFacts): "gone" | "conflict" | "write" | "already" {
  if (!f.found) return "gone";
  if (f.version !== f.sentVersion) return "conflict";
  if (f.current === f.base) return "write";
  return f.current === f.written ? "already" : "conflict";
}

/** "Already saved" writes nothing: it answers `saved`, or `written` when the document has unsaved changes (the text is in the buffer, not on disk). */
export const answerForAlready = (documentIsDirty: boolean): "saved" | "written" => (documentIsDirty ? "written" : "saved");

/**
 * The answer after `applyEdit`: it refused, a conflict (nothing was written); it applied and the document was saved, `saved`; it applied and the document was
 * not saved (it was dirty before, E4: the unsaved work of the user's own is never saved for them, or the save failed), `written`: in the buffer, not on disk.
 */
export function outcomeAfterApply(applied: boolean, saved: boolean): "saved" | "written" | "conflict" {
  if (!applied) return "conflict";
  return saved ? "saved" : "written";
}

/** The text of a file with the draft in place of `[from, to)`: what "Show Diff" puts beside the file. */
export function withDraft(file: string, from: number, to: number, written: string): string {
  return file.slice(0, from) + written + file.slice(to);
}

/**
 * The name the question uses (§7): the node's own name from its key, `path::Outer.inner` → `inner`, `path::f#2` → `f` (the card's name). A class that holds methods
 * is asked about by its own name even when it is a method's ID that goes: the text does not say whose it was.
 */
export function nodeName(key: string): string {
  const qualname = key.includes("::") ? key.slice(key.indexOf("::") + 2) : key;
  return qualname.replace(/#\d+$/, "").split(".").pop() || qualname;
}

/** The modal's text, and the button that goes on (Cancel is VS Code's own). */
export const idQuestion = (key: string): string => `This removes ${nodeName(key)}'s ID: its docs will be Missing.`;
export const SAVE_ANYWAY = "Save Anyway";
