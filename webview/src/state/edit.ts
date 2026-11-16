// What typing in a card, and its Save and Discard, do (docs/17-edit-in-canvas.md §5, 9.7). The rules are `draftRules.ts`; the store is `drafts.ts`. Plain functions, so Monaco's
// handlers, a button and the test suite's `viewCommand`s do the same thing.

import type { Range } from "@vnoc/shared/messages.ts";

import type { Placed } from "../canvas/layout.ts";
import { request } from "../host.ts";
import { unsavedDrafts, nextCard } from "../canvas/top-bar/unsavedDrafts.ts";
import { canSave, docBaselineOf, draftFrom, indexMovedOn, markKey, noticeFor, savedAs, settleTyping } from "./draftRules.ts";
import { PAGE } from "./page.ts";
import { useData } from "./data.ts";
import { dropDraft, putDraft, setBaseline, setDraftState, setElsewhere, setNotice, setRefused, setSettling, useDrafts } from "./drafts.ts";
import { dispatch } from "./session.ts";
import { requestCodeFocus, setDocsOpen } from "./view.ts";

/** What an edit starts from: the card, and the code it shows (a call card's is its callee's). */
export interface CardRef {
  id: string;
  key: string | null;
  path: string;
  range: Range | null;
}

export const cardRef = (n: Pick<Placed, "id" | "key" | "path" | "range">): CardRef => ({ id: n.id, key: n.key, path: n.path, range: n.range });

/**
 * Typing in the editor of a card (`id`: the card's id; `key`: its node) or of a doc's source view (`id` and `key`: the doc's path): the text is what Monaco holds.
 * - A draft of this node exists and this card holds it: the draft follows the text, and is dropped when the text is the original again (`settleTyping`).
 * - Another card holds the node's draft: nothing (that card's Monaco is read-only).
 * - No draft: the first text that differs from the card's baseline creates the draft from it, locally, with no request (`draftFrom`). No baseline (the file has unsaved changes, the
 *   code is not read yet): nothing.
 * The test suite's `setDraft` is this too: what typing makes.
 */
export function typeIn(id: string, key: string, text: string): void {
  const { drafts, baselines } = useDrafts.getState();
  const held = drafts[key];
  if (held) {
    if (held.kind === "doc" || held.card === id) useDrafts.setState(({ drafts: now }) => ({ drafts: settleTyping(now, key, text) }));
    return;
  }
  const fresh = baselines[id] ? draftFrom(baselines[id], text) : null;
  if (fresh) putDraft(fresh);
}

/** Brings a card into view with the session's `reveal`, which a sidebar click sends too: its parents open so it is drawn, and the canvas centres on it and pulses it. */
export const revealCard = (id: string): void => dispatch({ type: "reveal", id, canvas: true });

/** The last card the bar's chip took you to: the next click goes to the one after it. */
let lastCycled: string | null = null;

/** The bar's "N unsaved drafts": reveals the next draft's card, cycling through them. */
export function revealNextDraft(): void {
  const found = unsavedDrafts(useDrafts.getState().drafts);
  const next = found ? nextCard(found.cards, lastCycled) : null;
  if (next === null) return;
  lastCycled = next;
  revealCard(next);
  // a doc's draft shows in the doc panel: open it, for the node the draft was started on (the reveal selected it)
  if (Object.values(useDrafts.getState().drafts).some((d) => d.kind === "doc" && d.card === next)) setDocsOpen(true);
}

/** The longest the card waits for the index to catch up with a save. */
const INDEX_WAIT_MS = 3000;

/**
 * Resolves when the index has moved on from `before` (a newer revision, or another run of the core), or after a few seconds: a save that changed no byte may not
 * bump it. Start it before the save is asked for, so an index that is quick is not missed.
 */
function indexCaughtUp(before: ReturnType<typeof useData.getState>["structure"]): { done: Promise<void>; cancel: () => void } {
  let done!: () => void;
  const promise = new Promise<void>((resolve) => {
    const timer = setTimeout(finish, INDEX_WAIT_MS);
    const stop = useData.subscribe((s) => indexMovedOn(before, s.structure) && finish());
    function finish() {
      clearTimeout(timer);
      stop();
      resolve();
    }
    done = finish;
  });
  return { done: promise, cancel: done };
}

/**
 * "Save `<file>` to edit here" (E5, up front): the host saves the document, and the card's range, which came from the last save, is stale until the index has read the new one. So the
 * card stays read-only (`settling`) until a newer index has come, or a few seconds have passed; it reads its code again by itself when the file's dirty state flips and its range moves.
 */
export async function saveFile(card: CardRef): Promise<void> {
  if (card.range === null) return;
  setSettling(card.id, true);
  const moved = indexCaughtUp(useData.getState().structure);
  try {
    await request("code", { path: card.path, range: card.range, saveFirst: true }); // the answer is for the old range: only the save is wanted
  } catch {
    moved.cancel(); // the save failed or was refused: the card says so again, as a dirty file
  }
  await moved.done;
  setSettling(card.id, false);
}

/** Discard: the draft goes, and the card reads its code again (`NodeCode` does it when the draft is gone); a doc shows the rendered Markdown again. */
export function discardDraft(key: string): void {
  const d = useDrafts.getState().drafts[key];
  if (d) setNotice(markKey(d), null);
  dropDraft(key);
  if (d?.kind === "doc") closeDocSource(d.path); // a doc goes back to the rendered Markdown
}

/**
 * Save (the button, and ⌘S in Monaco; docs/17-edit-in-canvas.md §6): nothing is sent when the draft equals its start. Otherwise the draft is `saving` (Save and
 * Discard disabled) while the host checks and writes.
 * - `saved`: the card keeps showing the saved text, read-only, until the index has moved on (read at the old range from the new text it would show the wrong
 *   lines), then the draft goes and the card reads its code at the new range.
 * - `written` (the file had unsaved changes, so it was not saved): the index will not move, so the draft goes at once, and the card reads its code and says
 *   "unsaved in editor" (9.1) and "Written into the file, not saved".
 * - `conflict` or `gone`: nothing was written; the draft stays, in that state, with its line and buttons.
 * - `cancelled`: the draft takes the function's `ID:` away and the user said Cancel to the host's question: back to `editing`, nothing else changes.
 * - an error: back to `editing`, with the reason.
 */
export async function saveDraft(key: string): Promise<void> {
  const d = useDrafts.getState().drafts[key];
  if (!canSave(d)) return;
  const file = d.path.slice(d.path.lastIndexOf("/") + 1);
  const mark = markKey(d);
  setNotice(mark, null);
  setDraftState(key, "saving");
  // a doc is not in the index: there is nothing to wait for, and the draft goes as soon as the host has written it (§8)
  const moved = d.kind === "doc" ? { done: Promise.resolve(), cancel: () => {} } : indexCaughtUp(useData.getState().structure);
  let answer;
  try {
    answer = await request("editSave", { ...(d.kind ? { kind: d.kind } : {}), key, path: d.path, range: d.range, version: d.version, base: d.base, prefix: d.prefix, eol: d.eol, text: d.text });
  } catch (e) {
    moved.cancel();
    setDraftState(key, "editing");
    setNotice(mark, noticeFor("error", file, (e as Error).message));
    return;
  }
  const outcome = savedAs(answer);
  if (outcome === "cancelled") {
    moved.cancel();
    setDraftState(key, "editing"); // the question about the ID was answered Cancel (§7): nothing was written, and the draft is as it was
    return;
  }
  if (outcome === "conflict" || outcome === "gone") {
    moved.cancel();
    setDraftState(key, outcome);
    return;
  }
  if (outcome === "written") {
    moved.cancel();
    setNotice(mark, noticeFor("written", file));
  } else await moved.done;
  dropDraft(key);
  if (d.kind === "doc") closeDocSource(d.path); // saved: the panel shows the rendered doc again, from the `docText` event
}

/** Reload, on a conflict: nothing of the draft is kept, and the card shows the file's text now, which it reads again once the draft is gone. */
export const reloadDraft = (key: string): void => discardDraft(key);

/** Show Diff, on a conflict: the file beside the file with the draft in the function's place. The draft stays. */
export function showDiff(key: string): void {
  const d = useDrafts.getState().drafts[key];
  if (d) void request("editDiff", { ...(d.kind ? { kind: d.kind } : {}), key, path: d.path, range: d.range, base: d.base, prefix: d.prefix, eol: d.eol, text: d.text }).catch(() => {});
}

/** Open in Editor, on a function that is gone (the file, where the draft started) or a doc that is gone (the `.md`). */
export function openInEditor(key: string): void {
  const d = useDrafts.getState().drafts[key];
  if (!d) return;
  // a doc is opened as a doc, from anywhere in the repository (`openAt` is for a file below the opened folder)
  void (d.kind === "doc" ? request("openDoc", { path: d.path }) : request("openAt", { path: d.path, range: d.range })).catch(() => {});
}

/** Copy Draft, on a function that is gone: the draft's text, as the card shows it. The host's clipboard: the page's may be blocked. */
export function copyDraft(key: string): void {
  const d = useDrafts.getState().drafts[key];
  if (d) void request("copyText", { text: d.text }).catch(() => {});
}

/** What the doc panel's pencil starts from (§8): the Markdown file's absolute path, and the node the panel shows it for (the draft's `card`). */
export interface DocRef {
  path: string;
  node: string;
}

/**
 * The doc panel's pencil (§8, 9.7): the panel shows the source of the `.md` in Monaco, with no draft yet. The host opens the document. If the `.md` has unsaved changes in a tab it says
 * so and the panel shows "Save `<file>` first" (`refused`); if the other page has a draft of it, `busy`, and the panel says which page; otherwise the doc's baseline is kept (the panel
 * shows its source, which takes typing: the first keystroke that differs starts the draft, as on a card) and the editor takes the keyboard. `saveFirst` is Save and Edit: there is no range
 * to wait for, so the host saves and answers at once. Asked again while the source is shown, it refreshes the baseline (the doc changed under it) without taking the keyboard.
 */
export async function openDocSource(doc: DocRef, saveFirst = false, refresh = false): Promise<"shown" | "refused" | "busy" | "ignored" | "failed"> {
  if (useDrafts.getState().drafts[doc.path]) return "ignored";
  setNotice(doc.path, null);
  try {
    const got = await request("editStart", { kind: "doc", path: doc.path, page: PAGE, saveFirst });
    if (useDrafts.getState().drafts[doc.path]) return "ignored"; // typed while the host answered: the draft has its own base
    if ("dirty" in got) {
      setElsewhere(doc.path, false);
      setRefused(doc.path, true);
      setBaseline(doc.path, null);
      return "refused";
    }
    if ("busy" in got) {
      setRefused(doc.path, false);
      setElsewhere(doc.path, true);
      setBaseline(doc.path, null);
      return "busy";
    }
    setRefused(doc.path, false);
    setElsewhere(doc.path, false);
    setBaseline(doc.path, docBaselineOf(doc, got));
    if (!refresh) requestCodeFocus(doc.path);
    return "shown";
  } catch {
    return "failed";
  }
}

/** The pencil again, with no draft: back to the rendered Markdown. */
export function closeDocSource(path: string): void {
  setBaseline(path, null);
}

/** Reload, on a conflict in a doc: nothing of the draft is kept, and the source shows the file's text now. */
export async function reloadDocDraft(doc: DocRef): Promise<void> {
  discardDraft(doc.path);
  await openDocSource(doc);
}
