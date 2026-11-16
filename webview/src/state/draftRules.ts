// The rules of a draft (docs/17-edit-in-canvas.md §4, §5, E6). Pure, so the table is tested; `drafts.ts` holds the store and `edit.ts` talks to the host.

import type { Range } from "@vnoc/shared/messages.ts";

import { type DraftState } from "../node-card/editState.ts";
import { dedent } from "../node-card/indent.ts";

/**
 * One function's unsaved edit. `key` is the structure node's key (a call card's callee's), so two cards on one function have one draft. `card` is the id of
 * the card that started it: the only card that shows it editing (9.5 refuses a second one with a message; until then the other card shows its code).
 * `path`, `range`, `version`, `base` and `prefix` are what the host said when it started: §6's checks compare against them at save time. `text` is what Monaco
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
  text: string;
  state: DraftState;
}
export type Drafts = Readonly<Record<string, Draft>>;

/** What Monaco shows when nothing was typed. */
export const startText = (d: Pick<Draft, "base" | "prefix">): string => dedent(d.base, d.prefix);

/** Whether the draft differs from its start: Save is enabled, and the card's dirty dot shows. */
export const isChanged = (d: Draft): boolean => d.text !== startText(d);

/** The draft of the function this card shows, if this card is the one that started it. */
export function draftFor(drafts: Drafts, card: { id: string; key: string | null }): Draft | undefined {
  const d = card.key === null ? undefined : drafts[card.key];
  return d && d.card === card.id ? d : undefined;
}

/** A new draft in `editing`, from what `editStart` answered. */
export function newDraft(card: { id: string; key: string; path: string; range: Range }, got: { text: string; version: number; prefix: string }): Draft {
  const d = { key: card.key, card: card.id, path: card.path, range: card.range, version: got.version, base: got.text, prefix: got.prefix, state: "editing" as const };
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
    out[key] = { ...(v as unknown as Draft), state: v.state === "saving" || !STATES.includes(v.state as string) ? "editing" : (v.state as DraftState) };
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
