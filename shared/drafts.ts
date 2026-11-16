// What the page and the host both know about a draft (docs/17-edit-in-canvas.md §4, §7, 9.5c). Pure. The page holds the drafts (E6); the host keeps a copy, fed by the page, and acts on it
// only when the canvas is closed or at start: these are the rules of that copy.

import type { Draft, DraftState, EditSaveResult, Page } from "./messages.ts";
import { dedent } from "./indent.ts";

/** What Monaco shows when nothing was typed. */
export const startText = (d: Pick<Draft, "base" | "prefix">): string => dedent(d.base, d.prefix);

const unix = (text: string): string => text.replace(/\r\n/g, "\n");

/** Whether the draft differs from its start: Save is enabled, and the card's dirty dot shows. Line endings do not count: Monaco gives its own back. */
export const isChanged = (d: Draft): boolean => unix(d.text) !== unix(startText(d));

/** What the host's answer to `editSave` means for the draft. */
export type SaveAnswer = "saved" | "written" | "conflict" | "gone" | "cancelled";
export function savedAs(answer: EditSaveResult): SaveAnswer {
  return "conflict" in answer ? "conflict" : "gone" in answer ? "gone" : "cancelled" in answer ? "cancelled" : "written" in answer ? "written" : "saved";
}

/**
 * Whether a draft counts when the canvas closes or at start (§7): there is something to lose. Changed (typed into, whatever state it is in, `saving` included), or a draft whose
 * save found a conflict or a function that is gone. An `editing` draft nothing was typed into does not: it is the file's own text.
 */
export const countsOnClose = (d: Draft): boolean => isChanged(d) || d.state === "conflict" || d.state === "gone";

/** The drafts of a list that count, in the list's order. */
export const countingDrafts = (drafts: readonly Draft[]): Draft[] => drafts.filter(countsOnClose);

const STATES: readonly string[] = ["editing", "saving", "conflict", "gone"];
const isPosition = (p: unknown): boolean => typeof p === "object" && p !== null && typeof (p as { line: unknown }).line === "number" && typeof (p as { character: unknown }).character === "number";

/**
 * A whole draft from something that was saved (`vscode.getState()`, the host's `workspaceState`), or null for anything that is not one. A state this version does not know comes back
 * `editing`, and an `eol` that is not `\r\n` comes back `\n` (a draft saved before it was there). `saving` stays `saving`: what it means after a page starts is `fromCopy`'s to say.
 */
export function readDraft(v: unknown): Draft | null {
  if (typeof v !== "object" || v === null) return null;
  const d = v as Record<string, unknown>;
  const range = d.range as { start: unknown; end: unknown } | undefined;
  const whole =
    typeof d.key === "string" && typeof d.card === "string" && typeof d.path === "string" && typeof d.version === "number" && typeof d.base === "string" && typeof d.prefix === "string" && typeof d.text === "string" && typeof d.state === "string" && range && isPosition(range.start) && isPosition(range.end);
  if (!whole) return null;
  const { kind: _kind, ...rest } = d as unknown as Draft;
  return { ...(d.kind === "doc" ? { kind: "doc" as const } : {}), ...rest, eol: d.eol === "\r\n" ? "\r\n" : "\n", state: STATES.includes(d.state as string) ? (d.state as DraftState) : "editing" };
}

/** The copy as `workspaceState` held it: a list of whole drafts, one per key (the first wins); anything else is an empty copy. */
export function readDrafts(saved: unknown): Draft[] {
  if (!Array.isArray(saved)) return [];
  const seen = new Set<string>();
  const out: Draft[] = [];
  for (const v of saved) {
    const d = readDraft(v);
    if (d && !seen.has(d.key)) (seen.add(d.key), out.push(d));
  }
  return out;
}

/** The drafts the host had saved, as a page starts them: by key, and a draft that was `saving` is `editing` (the request that was saving it is gone with the page, and Save is asked for again). */
export function fromCopy(copy: readonly Draft[]): Record<string, Draft> {
  return Object.fromEntries(copy.map((d) => [d.key, d.state === "saving" ? { ...d, state: "editing" as const } : d]));
}

/** What a page that starts holds: its own drafts when it has any (a webview reload keeps them), else the host's copy. A draft started in the moment between asking and the answer wins its key. */
export function adopt(own: Readonly<Record<string, Draft>>, copy: readonly Draft[]): Record<string, Draft> {
  return { ...fromCopy(copy), ...own };
}

/** The pages a notification speaks for (§8, 9.6): one of them, or both (at start, when both left drafts). */
export type Source = Page | "both";

/** `sourceOf(["canvas"])` → `canvas`; both → `both`; none → null (nothing to say). */
export function sourceOf(pages: readonly Page[]): Source | null {
  const has = (p: Page) => pages.includes(p);
  return has("canvas") && has("docs") ? "both" : has("canvas") ? "canvas" : has("docs") ? "docs" : null;
}

const FROM: Record<Source, string> = { canvas: "the canvas", docs: "the docs tab", both: "V-NOC" };

/** What the notification says (§7, §8): `N unsaved draft(s) from the canvas`, `… from the docs tab`, `… from V-NOC` (both). At close and at start it is the same sentence. */
export const closeNotice = (n: number, from: Source = "canvas"): string => `${n} unsaved ${n === 1 ? "draft" : "drafts"} from ${FROM[from]}`;
export const startNotice = closeNotice;

/** The button that reopens what the drafts came from. */
export const REOPEN_CANVAS = "Reopen Canvas";
export const REOPEN_DOCS = "Reopen Docs Tab";
export const REOPEN = "Reopen";
export const reopenButton = (from: Source): string => (from === "canvas" ? REOPEN_CANVAS : from === "docs" ? REOPEN_DOCS : REOPEN);

/** The second notification, after Save All left some (§7): `K could not be saved: Reopen Canvas to see them`. */
export const failedNotice = (k: number, from: Source = "canvas"): string => `${k} could not be saved: ${reopenButton(from)} to see ${k === 1 ? "it" : "them"}`;

/** The buttons of the notifications. */
export const SAVE_ALL = "Save All";
export const DISCARD = "Discard";
/** At close: all three. At start: Save All is not offered (the page is not there to say what a save did, and a window that has just opened has not read the code). */
export const closeButtons = (from: Source): string[] => [SAVE_ALL, reopenButton(from), DISCARD];
export const startButtons = (from: Source): string[] => [reopenButton(from), DISCARD];

/** The host's copy of the drafts: each page's own list (§8, 9.6). One draft per doc across both pages is the host's rule (`busyWith`). */
export type Copy = Record<Page, Draft[]>;
export const emptyCopy = (): Copy => ({ canvas: [], docs: [] });
export const PAGES: readonly Page[] = ["canvas", "docs"];

/** The copy as `workspaceState` held it: a list per page. 9.5c kept a plain list, which was the canvas's. */
export function readCopy(saved: unknown): Copy {
  if (Array.isArray(saved)) return { canvas: readDrafts(saved), docs: [] };
  const o = typeof saved === "object" && saved !== null ? (saved as Record<string, unknown>) : {};
  return { canvas: readDrafts(o.canvas), docs: readDrafts(o.docs) };
}

/** The copy with one page's list replaced: the other page's is kept. */
export const withPage = (copy: Copy, page: Page, drafts: Draft[]): Copy => ({ ...copy, [page]: drafts });

/** Whether a copy holds no draft at all (the memento is cleared then). */
export const isEmptyCopy = (copy: Copy): boolean => copy.canvas.length === 0 && copy.docs.length === 0;

/**
 * Whether `asking` may not edit the doc at `path` because the other page has a draft of it (§8: one draft per doc, across the canvas and the docs tab, which are two pages that
 * cannot see each other). The other page must be there: a canvas that was closed with a draft the user has not answered still holds it in the copy, but nobody is editing it.
 */
export function busyWith(copy: Copy, path: string, asking: Page, open: (page: Page) => boolean): boolean {
  const other: Page = asking === "canvas" ? "docs" : "canvas";
  return open(other) && copy[other].some((d) => d.kind === "doc" && d.path === path);
}

/** The line a panel says when its Edit was refused for this (§8). */
export const busyText = (asking: Page): string => `Already being edited in ${asking === "canvas" ? "the docs tab" : "the canvas"}`;

/**
 * What Save All leaves in the copy. `answers[i]` is what `editSave` said for `drafts[i]`, `"error"` when the request threw, or `undefined` when it was not asked (the draft did not count). Saved or written: the draft is handled and goes. A conflict or a gone
 * function stays, in that state. A save the user cancelled (the question about the ID) or one that failed stays as it was. A draft that did not count (nothing typed) is handled too: it has nothing to
 * save and goes. `failed` is how many stay: the K of the second notification.
 */
export function settle(drafts: readonly Draft[], answers: readonly (SaveAnswer | "error" | undefined)[]): { remaining: Draft[]; failed: number } {
  const remaining: Draft[] = [];
  drafts.forEach((d, i) => {
    const answer = answers[i];
    if (answer === undefined || answer === "saved" || answer === "written") return;
    remaining.push(answer === "conflict" || answer === "gone" ? { ...d, state: answer } : d);
  });
  return { remaining, failed: remaining.length };
}
