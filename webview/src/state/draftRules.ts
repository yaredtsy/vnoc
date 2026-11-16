// The rules of a draft (docs/17-edit-in-canvas.md §4, §5, E6). Pure, so the table is tested; `drafts.ts` holds the store and `edit.ts` talks to the host.

import type { Draft, Range } from "@vnoc/shared/messages.ts";
import { countsOnClose, isChanged, readDraft, savedAs, startText } from "@vnoc/shared/drafts.ts";

import { type DraftState } from "../node-card/editState.ts";

export type { Draft };
// the rules both sides share (the page holds the drafts, the host keeps a copy): `shared/drafts.ts`
export { isChanged, savedAs, startText };

export type Drafts = Readonly<Record<string, Draft>>;

/** The draft of the function this card shows, if this card is the one that started it. */
export function draftFor(drafts: Drafts, card: { id: string; key: string | null }): Draft | undefined {
  const d = card.key === null ? undefined : drafts[card.key];
  return d && d.card === card.id ? d : undefined;
}

/**
 * What an Edit on a card does about the drafts there are (§7, 9.5): a node has one draft (E6), so the card that holds it is already editing (`same`: nothing to do) and
 * any other card on that node, a call card or the structure card, is refused (`other`), with the holder to bring into view.
 */
export type EditBlock = null | { kind: "same" } | { kind: "other"; holder: string };

export function editBlock(drafts: Drafts, card: { id: string; key: string | null }): EditBlock {
  const d = card.key === null ? undefined : drafts[card.key];
  if (!d) return null;
  return d.card === card.id ? { kind: "same" } : { kind: "other", holder: d.card };
}

/** The line a refused card shows (§7). */
export const ELSEWHERE_TEXT = "Already being edited in another card";

/** It shows while the card was refused and the draft that refused it is still there in another card: once that draft is saved or discarded the line goes. */
export const showElsewhere = (refused: boolean, drafts: Drafts, card: { id: string; key: string | null }): boolean => refused && editBlock(drafts, card)?.kind === "other";

/** A draft the user would lose: typed into, or in a state that waits on the user (saving, or a save that found a conflict or a function that is gone). The bar counts these (§7). */
export const isUnsaved = (d: Draft): boolean => countsOnClose(d) || d.state === "saving";

/** A new draft in `editing`, from what `editStart` answered. */
export function newDraft(card: { id: string; key: string; path: string; range: Range }, got: { text: string; version: number; prefix: string; eol: "\n" | "\r\n" }): Draft {
  const d = { key: card.key, card: card.id, path: card.path, range: card.range, version: got.version, base: got.text, prefix: got.prefix, eol: got.eol, state: "editing" as const };
  return { ...d, text: startText(d) };
}

/**
 * A new draft of a doc (§8), in `editing`, from what `editStart` answered: the whole file as the range, no prefix. `path` is the Markdown file's absolute path and the draft's key;
 * `node` is the node the panel showed it for (`card`), which is what a page shows again to bring the draft back.
 */
export function newDocDraft(doc: { path: string; node: string }, got: { text: string; version: number; eol: "\n" | "\r\n"; range?: Range }): Draft {
  const range = got.range ?? { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } };
  const d = { kind: "doc" as const, key: doc.path, card: doc.node, path: doc.path, range, version: got.version, base: got.text, prefix: "", eol: got.eol, state: "editing" as const };
  return { ...d, text: startText(d) };
}

/** The key of a draft's notice and refusal marks: a card's id, or a doc's path (a doc has no card of its own: its `card` is the node, which a node's docs share). */
export const markKey = (d: Pick<Draft, "kind" | "card" | "path">): string => (d.kind === "doc" ? d.path : d.card);

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

/**
 * What `vscode.getState()` held (E6: the drafts go in `Saved` with the view). A draft that was `saving` comes back as `editing`: the request that was saving it
 * is gone with the page, and the save is asked for again. An entry that is not a whole draft is dropped, and so is anything that is not an object.
 */
export function restoreDrafts(saved: unknown): Drafts {
  if (typeof saved !== "object" || saved === null) return {};
  const out: Record<string, Draft> = {};
  for (const [key, v] of Object.entries(saved)) {
    const d = readDraft(v);
    if (d && d.key === key) out[key] = d.state === "saving" ? { ...d, state: "editing" } : d;
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

/**
 * What the page tells the host about its drafts when they change (§7, 9.5c: the host keeps a copy). `now`: a draft started, went, or changed in anything but its text (a state, a
 * range); the copy must not lag a Discard or a save. `later`: only text changed: typing, sent 500 ms after the last keystroke. `none`: nothing the host keeps changed.
 */
export function feedFor(prev: Drafts, next: Drafts): "none" | "now" | "later" {
  if (prev === next) return "none";
  const keys = Object.keys(next);
  if (keys.length !== Object.keys(prev).length) return "now";
  let typing = false;
  for (const key of keys) {
    const a = prev[key];
    const b = next[key];
    if (!a || a === b) continue;
    const { text: textA, ...restA } = a;
    const { text: textB, ...restB } = b;
    if (JSON.stringify(restA) !== JSON.stringify(restB)) return "now";
    if (textA !== textB) typing = true;
  }
  return typing ? "later" : "none";
}
