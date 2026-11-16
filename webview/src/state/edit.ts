// What the card's Edit, Save and Discard do (docs/17-edit-in-canvas.md §5). The rules are `draftRules.ts`; the store is `drafts.ts`. Plain functions, so a button
// and a menu command (`viewCommand` "edit") start an edit the same way.

import type { Range } from "@vnoc/shared/messages.ts";
import { findNode } from "@vnoc/shared/tree.ts";

import type { Placed } from "../canvas/layout.ts";
import { request } from "../host.ts";
import { unsavedDrafts, nextCard } from "../canvas/top-bar/unsavedDrafts.ts";
import { canSave, editBlock, indexMovedOn, newDraft, noticeFor, savedAs } from "./draftRules.ts";
import { useData } from "./data.ts";
import { dropDraft, putDraft, setDraftState, setElsewhere, setNotice, setRefused, useDrafts } from "./drafts.ts";
import { dispatch } from "./session.ts";
import { setCodeOpen } from "./view.ts";

/** What an edit starts from: the card, and the code it shows (a call card's is its callee's). */
export interface CardRef {
  id: string;
  key: string | null;
  path: string;
  range: Range | null;
}

export const cardRef = (n: Pick<Placed, "id" | "key" | "path" | "range">): CardRef => ({ id: n.id, key: n.key, path: n.path, range: n.range });

/**
 * Edit: the code is shown (the card needs it to say anything), then the host opens the document. If the file has unsaved changes the host says so and the card
 * shows E5's message (`refused`); otherwise a draft starts in `editing`. A node has one draft (§7): on the card that holds it Edit does nothing (`ignored`); on any
 * other card on the same node it is refused (`elsewhere`): that card says so, and the holder is brought into view.
 */
export async function beginEdit(card: CardRef, saveFirst = false): Promise<"started" | "refused" | "elsewhere" | "ignored" | "failed"> {
  if (card.key === null || card.range === null) return "ignored";
  setCodeOpen(card.id, true);
  const block = editBlock(useDrafts.getState().drafts, card);
  if (block?.kind === "same") return "ignored";
  if (block) {
    setElsewhere(card.id, true);
    revealCard(block.holder);
    return "elsewhere";
  }
  setElsewhere(card.id, false);
  setNotice(card.id, null);
  try {
    const got = await request("editStart", { path: card.path, range: card.range, saveFirst });
    if ("dirty" in got) {
      setRefused(card.id, true);
      return "refused";
    }
    if ("busy" in got) return "failed"; // only a doc can be busy (§8); a function is edited in the canvas alone
    setRefused(card.id, false);
    putDraft(newDraft({ id: card.id, key: card.key, path: card.path, range: card.range }, got));
    return "started";
  } catch {
    return "failed";
  }
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
 * "Save and Edit": the host saves the document, and the card's range, which came from the last save, is stale until the index has read the new one. So the draft
 * does not start from the host's answer: it waits for a newer index, reads the node's range again, and starts from step 1 of §5 with that.
 */
export async function saveAndEdit(card: CardRef): Promise<void> {
  if (card.key === null || card.range === null) return;
  const moved = indexCaughtUp(useData.getState().structure);
  try {
    await request("editStart", { path: card.path, range: card.range, saveFirst: true }); // the answer is for the old range: only the save is wanted
  } catch {
    // the save failed or was refused: the second step says so again, as a refusal
  }
  await moved.done;
  const root = useData.getState().structure?.root;
  const fresh = (root && findNode(root, card.key)?.range) || card.range;
  await beginEdit({ ...card, range: fresh });
}

/** Discard: the draft goes, and the card reads its code again (`NodeCode` does it when the draft is gone). */
export function discardDraft(key: string): void {
  const d = useDrafts.getState().drafts[key];
  if (d) setNotice(d.card, null);
  dropDraft(key);
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
  setNotice(d.card, null);
  setDraftState(key, "saving");
  const moved = indexCaughtUp(useData.getState().structure);
  let answer;
  try {
    answer = await request("editSave", { key, path: d.path, range: d.range, version: d.version, base: d.base, prefix: d.prefix, eol: d.eol, text: d.text });
  } catch (e) {
    moved.cancel();
    setDraftState(key, "editing");
    setNotice(d.card, noticeFor("error", file, (e as Error).message));
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
    setNotice(d.card, noticeFor("written", file));
  } else await moved.done;
  dropDraft(key);
}

/** Reload, on a conflict: nothing of the draft is kept. The draft goes and a new one starts from the file's text now, at the node's range now. */
export async function reloadDraft(card: CardRef): Promise<void> {
  if (card.key === null) return;
  discardDraft(card.key);
  await beginEdit(card);
}

/** Show Diff, on a conflict: the file beside the file with the draft in the function's place. The draft stays. */
export function showDiff(key: string): void {
  const d = useDrafts.getState().drafts[key];
  if (d) void request("editDiff", { key, path: d.path, range: d.range, base: d.base, prefix: d.prefix, eol: d.eol, text: d.text }).catch(() => {});
}

/** Open in Editor, on a function that is gone: the file, where the draft started. */
export function openInEditor(key: string): void {
  const d = useDrafts.getState().drafts[key];
  if (d) void request("openAt", { path: d.path, range: d.range }).catch(() => {});
}

/** Copy Draft, on a function that is gone: the draft's text, as the card shows it. The host's clipboard: the page's may be blocked. */
export function copyDraft(key: string): void {
  const d = useDrafts.getState().drafts[key];
  if (d) void request("copyText", { text: d.text }).catch(() => {});
}
