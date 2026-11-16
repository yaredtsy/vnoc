// What the card's Edit, Save and Discard do (docs/17-edit-in-canvas.md §5). The rules are `draftRules.ts`; the store is `drafts.ts`. Plain functions, so a button
// and a menu command (`viewCommand` "edit") start an edit the same way.

import type { Range } from "@vnoc/shared/messages.ts";
import { findNode } from "@vnoc/shared/tree.ts";

import type { Placed } from "../canvas/layout.ts";
import { request } from "../host.ts";
import { canSave, indexMovedOn, newDraft, noticeFor, savedAs } from "./draftRules.ts";
import { useData } from "./data.ts";
import { dropDraft, putDraft, setDraftState, setNotice, setRefused, useDrafts } from "./drafts.ts";
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
 * shows E5's message (`refused`); otherwise a draft starts in `editing`. A node that already has a draft is left alone: 9.5 tells the user why.
 */
export async function beginEdit(card: CardRef, saveFirst = false): Promise<"started" | "refused" | "ignored" | "failed"> {
  if (card.key === null || card.range === null) return "ignored";
  setCodeOpen(card.id, true);
  if (useDrafts.getState().drafts[card.key]) return "ignored";
  setNotice(card.id, null);
  try {
    const got = await request("editStart", { path: card.path, range: card.range, saveFirst });
    if ("dirty" in got) {
      setRefused(card.id, true);
      return "refused";
    }
    setRefused(card.id, false);
    putDraft(newDraft({ id: card.id, key: card.key, path: card.path, range: card.range }, got));
    return "started";
  } catch {
    return "failed";
  }
}

/** The longest the card waits for the index to catch up with a save. */
const INDEX_WAIT_MS = 3000;

/**
 * Resolves when the index has moved on from `before` (a newer revision, or another run of the core), or after a few seconds: a save that changed no byte may not
 * bump it. Start it before the save is asked for, so an index that is quick is not missed.
 */
function indexCaughtUp(before: ReturnType<typeof useData.getState>["structure"]): Promise<void> {
  return new Promise<void>((resolve) => {
    const timer = setTimeout(done, INDEX_WAIT_MS);
    const stop = useData.subscribe((s) => indexMovedOn(before, s.structure) && done());
    function done() {
      clearTimeout(timer);
      stop();
      resolve();
    }
  });
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
  await moved;
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
 * Discard disabled) while the host checks and writes. `saved` or `written`: the card keeps showing the saved text, read-only, until the index has moved on (read
 * at the old range from the new text it would show the wrong lines), then the draft goes and the card reads its code at the new range. `conflict`: the draft
 * stays, in `conflict`. An error: back to `editing`, with the reason.
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
    answer = await request("editSave", { path: d.path, range: d.range, version: d.version, base: d.base, prefix: d.prefix, eol: d.eol, text: d.text });
  } catch (e) {
    setDraftState(key, "editing");
    setNotice(d.card, noticeFor("error", file, (e as Error).message));
    return;
  }
  const outcome = savedAs(answer);
  if (outcome === "conflict") {
    setDraftState(key, "conflict");
    setNotice(d.card, noticeFor("conflict", file));
    return;
  }
  await moved;
  if (outcome === "written") setNotice(d.card, noticeFor("written", file));
  dropDraft(key);
}
