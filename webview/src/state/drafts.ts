// The drafts (docs/17-edit-in-canvas.md E6): one function's unsaved edit each, by the function's key. The drafts are saved with the view, by `view.ts`'s one
// subscriber (`vscode.setState` keeps one object per webview: a second call would wipe the other, measured in step 9.0), so they survive hiding the tab and a
// reload of the webviews. `refused` and `elsewhere` are not saved: the cards whose Edit the host refused because the file has unsaved changes (E5), and the cards whose
// Edit was refused because another card holds the node's draft (§7).
// The rules are `draftRules.ts`; this only holds the result. The things that talk to the host are `edit.ts`.

import { create } from "zustand";

import { vscode } from "../host.ts";
import type { DraftState } from "../node-card/editState.ts";
import { type Draft, type Drafts, type Notice, noticeAfterFileState, restoreDrafts, typed, withDraft, withState, without } from "./draftRules.ts";

export const useDrafts = create<{ drafts: Drafts; refused: ReadonlySet<string>; elsewhere: ReadonlySet<string>; notices: Readonly<Record<string, Notice>> }>(() => ({
  drafts: restoreDrafts(vscode.getState<{ drafts?: unknown }>()?.drafts),
  refused: new Set(),
  elsewhere: new Set(),
  notices: {},
}));

export const putDraft = (d: Draft) => useDrafts.setState(({ drafts }) => ({ drafts: withDraft(drafts, d) }));
export const typeInto = (key: string, text: string) => useDrafts.setState(({ drafts }) => ({ drafts: typed(drafts, key, text) }));
export const dropDraft = (key: string) => useDrafts.setState(({ drafts }) => ({ drafts: without(drafts, key) }));
export const setDraftState = (key: string, state: DraftState) => useDrafts.setState(({ drafts }) => ({ drafts: withState(drafts, key, state) }));

/** What the last Save did, in the line above the card's code (the card's id: it outlives the draft when the text was written but not saved). Not saved. */
export function setNotice(card: string, notice: Notice | null): void {
  useDrafts.setState(({ notices }) => {
    if (!notice && !notices[card]) return {};
    const { [card]: _gone, ...rest } = notices;
    return { notices: notice ? { ...rest, [card]: notice } : rest };
  });
}

/** The file of this card was saved or reverted: a "written, not saved" line has nothing left to wait for. */
export function fileStateChanged(card: string, fileDirty: boolean): void {
  const held = useDrafts.getState().notices[card];
  if (noticeAfterFileState(held, fileDirty) !== held) setNotice(card, null);
}

/** The host refused to start an edit of this card (`refused`), or it no longer does. */
export function setRefused(card: string, refused: boolean): void {
  useDrafts.setState(({ refused: held }) => {
    if (held.has(card) === refused) return {};
    const next = new Set(held);
    if (refused) next.add(card);
    else next.delete(card);
    return { refused: next };
  });
}

/** Edit on this card was refused because another card holds the node's draft (`elsewhere`), or no longer is. */
export function setElsewhere(card: string, refused: boolean): void {
  useDrafts.setState(({ elsewhere: held }) => {
    if (held.has(card) === refused) return {};
    const next = new Set(held);
    if (refused) next.add(card);
    else next.delete(card);
    return { elsewhere: next };
  });
}
