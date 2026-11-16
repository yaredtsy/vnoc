// What the page and the host both know about a draft (docs/17-edit-in-canvas.md §4, §7, 9.5c). Pure. The page holds the drafts (E6); the host keeps a copy, fed by the page, and acts on it
// only when the canvas is closed or at start: these are the rules of that copy.

import type { Draft, DraftState, EditSaveResult } from "./messages.ts";
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
  return { ...(d as unknown as Draft), eol: d.eol === "\r\n" ? "\r\n" : "\n", state: STATES.includes(d.state as string) ? (d.state as DraftState) : "editing" };
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

/** What the notification says (§7): `N unsaved draft(s) from the canvas`. At close and at start it is the same sentence. */
export const closeNotice = (n: number): string => `${n} unsaved ${n === 1 ? "draft" : "drafts"} from the canvas`;
export const startNotice = closeNotice;
/** The second notification, after Save All left some (§7). */
export const failedNotice = (k: number): string => `${k} could not be saved: Reopen Canvas to see ${k === 1 ? "it" : "them"}`;

/** The buttons of the notifications. */
export const SAVE_ALL = "Save All";
export const REOPEN_CANVAS = "Reopen Canvas";
export const DISCARD = "Discard";
/** At close: all three. At start: Save All is not offered (the canvas is not there to say what a save did, and a window that has just opened has not read the code). */
export const CLOSE_BUTTONS = [SAVE_ALL, REOPEN_CANVAS, DISCARD] as const;
export const START_BUTTONS = [REOPEN_CANVAS, DISCARD] as const;

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
