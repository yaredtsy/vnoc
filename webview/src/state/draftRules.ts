// The rules of a draft (docs/17-edit-in-canvas.md §4, §5, E6). Pure, so the table is tested; `drafts.ts` holds the store and `edit.ts` talks to the host.

import type { CodeResult, Draft, Range } from "@vnoc/shared/messages.ts";
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
 * Which card a node's draft belongs to (§7): a node has one draft (E6), so the card that holds it is `same`, and any other card on that node, a call card or the structure card,
 * is `other`: it is read-only while the draft exists (9.7), and names the holder, which a click brings into view.
 */
export type EditBlock = null | { kind: "same" } | { kind: "other"; holder: string };

export function editBlock(drafts: Drafts, card: { id: string; key: string | null }): EditBlock {
  const d = card.key === null ? undefined : drafts[card.key];
  if (!d) return null;
  return d.card === card.id ? { kind: "same" } : { kind: "other", holder: d.card };
}

/** The line a card shows while another card holds its node's draft (§7, 9.7). */
export const ELSEWHERE_TEXT = "Being edited in another card";

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

/**
 * Typing into a draft (9.7): **a draft exists only while its text differs from `base`**. Typing back to the original, or undoing to it, drops the draft, and the card is `reading`
 * again. Only an `editing` draft is typed into (a `saving`, `conflict` or `gone` one keeps its text), and the same object comes back when nothing changed.
 */
export function settleTyping(drafts: Drafts, key: string, text: string): Drafts {
  const next = typed(drafts, key, text);
  const d = next[key];
  return d && d.state === "editing" && !isChanged(d) ? without(next, key) : next;
}

/**
 * What a card (or a doc's source view) knows of its code when it was last read, with no draft (9.7): everything a draft starts from. The first keystroke that makes the text differ from
 * `text` creates the draft from it, with no request. `card` is the card that shows it (a doc's is the node), `key` the node (a doc's is its path).
 */
export interface Baseline {
  kind?: "doc";
  card: string;
  key: string;
  path: string;
  range: Range;
  version: number;
  text: string;
  prefix: string;
  eol: "\n" | "\r\n";
}

/** A card's baseline, from what `code` answered for its range. */
export const baselineOf = (card: { id: string; key: string; path: string; range: Range }, got: Pick<CodeResult, "text" | "version" | "prefix" | "eol">): Baseline => ({
  card: card.id, key: card.key, path: card.path, range: card.range, version: got.version, text: got.text, prefix: got.prefix, eol: got.eol,
});

/** A doc's baseline, from what the pencil's `editStart` answered: the whole file, no prefix. */
export const docBaselineOf = (doc: { path: string; node: string }, got: { text: string; version: number; eol: "\n" | "\r\n"; range: Range }): Baseline => ({
  kind: "doc", card: doc.node, key: doc.path, path: doc.path, range: got.range, version: got.version, text: got.text, prefix: "", eol: got.eol,
});

/** The draft the first keystroke makes: from the baseline, with the text as typed; null while the text is what the baseline says (nothing to lose, so no draft). */
export function draftFrom(b: Baseline, typedText: string): Draft | null {
  const start = { ...(b.kind ? { kind: b.kind } : {}), key: b.key, card: b.card, path: b.path, range: b.range, version: b.version, base: b.text, prefix: b.prefix, eol: b.eol, state: "editing" as const };
  const draft: Draft = { ...start, text: typedText };
  return isChanged(draft) ? draft : null;
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

// ── what a card's code shows above it, and whether it takes typing (§4, §5, 9.7) ─────────────────────────────────────────

/** What decides the line above a card's code and whether Monaco is writable. */
export interface CodeFacts {
  /** this card holds its node's draft */
  hasDraft: boolean;
  /** the card that holds the node's draft when it is another card */
  holder: string | null;
  /** the draft's state, when this card holds it */
  state: DraftState | null;
  /** the file has unsaved changes in a tab */
  dirty: boolean;
  /** the card pressed Save on the file and is waiting for the index to read it */
  settling: boolean;
  file: string;
  notice?: Notice;
}

export type CodeMark =
  /** another card holds this node's draft: read-only, and a click reveals that card */
  | { kind: "elsewhere"; holder: string; text: string }
  /** a draft in conflict or gone: what happened, and what can be done about it */
  | { kind: "state"; text: string; actions: DraftAction[] }
  /** E5, up front: the file has unsaved changes and no draft holds the card, so it is read-only until the file is saved: "Save `<file>` to edit here" and a Save button */
  | { kind: "saveFirst"; text: string }
  /** Save was pressed: the card waits for the index to read the file */
  | { kind: "saving"; text: string }
  /** what the last Save did: written but not saved, or why it failed */
  | { kind: "notice"; notice: Notice };

/**
 * The one line above a card's code, in this order: another card holds the node's draft; a draft's own state (conflict, gone); the file has unsaved changes (this line replaces
 * "unsaved in editor", and says what the last Save did when that is why the file is dirty); the file is being saved; what the last Save did.
 */
export function codeMark(f: CodeFacts): CodeMark | null {
  if (f.holder !== null) return { kind: "elsewhere", holder: f.holder, text: ELSEWHERE_TEXT };
  if (f.hasDraft) {
    const line = f.state ? stateLine(f.state, f.file) : null;
    if (line) return { kind: "state", ...line };
    return f.notice ? { kind: "notice", notice: f.notice } : null;
  }
  if (f.settling) return { kind: "saving", text: `Saving ${f.file}…` };
  if (f.dirty) return { kind: "saveFirst", text: f.notice?.kind === "written" ? `${f.notice.text} Save to edit here` : `Save ${f.file} to edit here` };
  return f.notice ? { kind: "notice", notice: f.notice } : null;
}

/**
 * Whether Monaco takes typing: while it shows a draft that is `editing`, or while it shows the code with no draft, the file saved, and the card not waiting for the index. Never while
 * another card holds the node's draft. (A dirty file is read-only because the card's range comes from the last save and can point at the wrong lines.)
 */
export function isWritable(f: Pick<CodeFacts, "hasDraft" | "holder" | "state" | "dirty" | "settling">): boolean {
  if (f.holder !== null) return false;
  if (f.hasDraft) return f.state === "editing";
  return !f.dirty && !f.settling;
}
