// The rules of a draft (docs/17-edit-in-canvas.md §4, §5, E6). Pure, so the table is tested; `drafts.ts` holds the store and `edit.ts` talks to the host.

import type { EditSaveResult, Range } from "@vnoc/shared/messages.ts";

import { type DraftState } from "../node-card/editState.ts";
import { dedent } from "@vnoc/shared/indent.ts";

/**
 * One function's unsaved edit. `key` is the structure node's key (a call card's callee's), so two cards on one function have one draft. `card` is the id of
 * the card that started it: the only card that shows it editing (9.5 refuses a second one with a message; until then the other card shows its code).
 * `path`, `range`, `version`, `base`, `prefix` and `eol` are what the host said when it started: §6's checks compare against them at save time. `text` is what Monaco
 * holds, the first line's indent taken off the others (`dedent(base, prefix)` when nothing was typed).
 */
export interface Draft {
  key: string;
  card: string;
  path: string;
  range: Range;
  version: number;
  base: string;
  prefix: string;
  /** the document's line ending, as `editStart` said; a draft saved before it was there has `"\n"` */
  eol: "\n" | "\r\n";
  text: string;
  state: DraftState;
}
export type Drafts = Readonly<Record<string, Draft>>;

/** What Monaco shows when nothing was typed. */
export const startText = (d: Pick<Draft, "base" | "prefix">): string => dedent(d.base, d.prefix);

const unix = (text: string): string => text.replace(/\r\n/g, "\n");

/** Whether the draft differs from its start: Save is enabled, and the card's dirty dot shows. Line endings do not count: Monaco gives its own back. */
export const isChanged = (d: Draft): boolean => unix(d.text) !== unix(startText(d));

/** The draft of the function this card shows, if this card is the one that started it. */
export function draftFor(drafts: Drafts, card: { id: string; key: string | null }): Draft | undefined {
  const d = card.key === null ? undefined : drafts[card.key];
  return d && d.card === card.id ? d : undefined;
}

/** A new draft in `editing`, from what `editStart` answered. */
export function newDraft(card: { id: string; key: string; path: string; range: Range }, got: { text: string; version: number; prefix: string; eol: "\n" | "\r\n" }): Draft {
  const d = { key: card.key, card: card.id, path: card.path, range: card.range, version: got.version, base: got.text, prefix: got.prefix, eol: got.eol, state: "editing" as const };
  return { ...d, text: startText(d) };
}

/** One draft per node: a second one for the same key is not started. */
export function withDraft(drafts: Drafts, d: Draft): Drafts {
  return drafts[d.key] ? drafts : { ...drafts, [d.key]: d };
}

/** What was typed; only while editing, and the same object when the text is the same (a store that sees no change does not save). */
export function typed(drafts: Drafts, key: string, text: string): Drafts {
  const d = drafts[key];
  return !d || d.state !== "editing" || d.text === text ? drafts : { ...drafts, [key]: { ...d, text } };
}

export function without(drafts: Drafts, key: string): Drafts {
  if (!drafts[key]) return drafts;
  const { [key]: _gone, ...rest } = drafts;
  return rest;
}

const STATES: readonly string[] = ["editing", "saving", "conflict", "gone"];
const isPosition = (p: unknown): boolean => typeof p === "object" && p !== null && typeof (p as { line: unknown }).line === "number" && typeof (p as { character: unknown }).character === "number";

/**
 * What `vscode.getState()` held (E6: the drafts go in `Saved` with the view). A draft that was `saving` comes back as `editing`: the request that was saving it
 * is gone with the page, and the save is asked for again. An entry that is not a whole draft is dropped, and so is anything that is not an object.
 */
export function restoreDrafts(saved: unknown): Drafts {
  if (typeof saved !== "object" || saved === null) return {};
  const out: Record<string, Draft> = {};
  for (const [key, v] of Object.entries(saved as Record<string, Record<string, unknown>>)) {
    if (typeof v !== "object" || v === null) continue;
    const range = v.range as { start: unknown; end: unknown } | undefined;
    const whole =
      v.key === key && typeof v.card === "string" && typeof v.path === "string" && typeof v.version === "number" && typeof v.base === "string" && typeof v.prefix === "string" && typeof v.text === "string" && typeof v.state === "string" && range && isPosition(range.start) && isPosition(range.end);
    if (!whole) continue;
    out[key] = { ...(v as unknown as Draft), eol: v.eol === "\r\n" ? "\r\n" : "\n", state: v.state === "saving" || !STATES.includes(v.state as string) ? "editing" : (v.state as DraftState) };
  }
  return out;
}

/** Whether the index has moved on since `before`: a newer revision of the same run of the core, or another run (a restarted core counts from the start again). */
export function indexMovedOn(before: { revision: number; epoch?: number } | null, now: { revision: number; epoch?: number } | null): boolean {
  if (!now) return false;
  if (!before) return true;
  return before.epoch !== now.epoch || now.revision > before.revision;
}

/** The refusal (E5, "Save `<file>` first") shows while the file is still dirty: once it is saved or reverted it goes, and Edit can be pressed again. */
export const showRefusal = (refused: boolean, fileDirty: boolean): boolean => refused && fileDirty;

/** The same draft in another state; the same object when it is already in it, or there is no such draft. */
export function withState(drafts: Drafts, key: string, state: DraftState): Drafts {
  const d = drafts[key];
  return !d || d.state === state ? drafts : { ...drafts, [key]: { ...d, state } };
}

/** Save does something only while editing a draft that differs from its start (the same rule as the button's: `cardButtons`). */
export const canSave = (d: Draft | undefined): d is Draft => d !== undefined && d.state === "editing" && isChanged(d);

/** What the host's answer to `editSave` means for the card. */
export function savedAs(answer: EditSaveResult): "saved" | "written" | "conflict" | "gone" {
  return "conflict" in answer ? "conflict" : "gone" in answer ? "gone" : "written" in answer ? "written" : "saved";
}

/**
 * The line above the code that says what the last Save did (9.3b, 9.4b). `written`: the text is in the file's buffer, not on disk, because the file was dirty (E4); it
 * stays until the file is saved. `error`: the request failed; it stays with the draft. A conflict and a gone function are states of the draft, and their line is `stateLine`.
 */
export interface Notice {
  kind: "written" | "error";
  text: string;
}

export function noticeFor(kind: Notice["kind"], file: string, detail?: string): Notice {
  switch (kind) {
    case "written":
      return { kind, text: `Written into ${file}, not saved.` };
    case "error":
      return { kind, text: detail ? `Could not save: ${detail}` : "Could not save" };
  }
}

/** The "written, not saved" line goes when the file is saved (or reverted): nothing is waiting any more. The error is the draft's. */
export const noticeAfterFileState = (notice: Notice | undefined, fileDirty: boolean): Notice | undefined => (notice?.kind === "written" && !fileDirty ? undefined : notice);

export type DraftAction = "reload" | "showDiff" | "openInEditor" | "copyDraft";

/**
 * A draft in `conflict` or `gone` says so in the card's body, with what can be done about it (§4); the header keeps `editing`'s buttons with Save disabled
 * (`cardButtons`). The line follows the draft's state, so it comes back with the draft after a reload. `editing` and `saving` have none.
 */
export function stateLine(state: DraftState, file: string): { text: string; actions: DraftAction[] } | null {
  switch (state) {
    case "conflict":
      return { text: "Changed in the editor since you started", actions: ["reload", "showDiff"] };
    case "gone":
      return { text: `Not found in ${file} any more`, actions: ["openInEditor", "copyDraft"] };
    default:
      return null;
  }
}

/** The button of an action, and what it says it does. Reload keeps nothing, and says so. */
export const ACTION_TEXT: Record<DraftAction, { label: string; tip: string }> = {
  reload: { label: "Reload", tip: "Start again from the file's text now: what you typed in the card is lost" },
  showDiff: { label: "Show Diff", tip: "Your draft against the file, in the diff editor" },
  openInEditor: { label: "Open in Editor", tip: "Open the file" },
  copyDraft: { label: "Copy Draft", tip: "Copy the draft's text to the clipboard" },
};
