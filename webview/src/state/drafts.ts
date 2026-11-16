// The drafts (docs/17-edit-in-canvas.md E6): one function's unsaved edit each, by the function's key. The drafts are saved with the view, by `view.ts`'s one
// subscriber (`vscode.setState` keeps one object per webview: a second call would wipe the other, measured in step 9.0), so they survive hiding the tab and a
// reload of the webviews. Not saved: `baselines` (what each card, and each doc's source view, knows of its code with no draft: the first keystroke that differs starts a draft from it,
// 9.7), `settling` (the cards waiting for the index after their Save), and, for the doc panel, `refused` and `elsewhere` (the pencil was refused because the `.md` has unsaved changes in a
// tab, or because the other page has a draft of it, §8).
// The rules are `draftRules.ts`; this only holds the result. The things that talk to the host are `edit.ts`.

import { create } from "zustand";

import { vscode } from "../host.ts";
import type { DraftState } from "../node-card/editState.ts";
import { type Baseline, type Draft, type Drafts, type Notice, noticeAfterFileState, restoreDrafts, typed, withDraft, withState, without } from "./draftRules.ts";

export const useDrafts = create<{ drafts: Drafts; refused: ReadonlySet<string>; elsewhere: ReadonlySet<string>; notices: Readonly<Record<string, Notice>>; baselines: Readonly<Record<string, Baseline>>; settling: ReadonlySet<string> }>(() => ({
  drafts: restoreDrafts(vscode.getState<{ drafts?: unknown }>()?.drafts),
  refused: new Set(),
  elsewhere: new Set(),
  notices: {},
  baselines: {},
  settling: new Set(),
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

/** What a card (by its id) or a doc's source view (by its path) knows of its code with no draft, or nothing (it is hidden, the file is dirty, a draft holds it). */
export function setBaseline(id: string, baseline: Baseline | null): void {
  useDrafts.setState(({ baselines }) => {
    if (!baseline && !baselines[id]) return {};
    const { [id]: _gone, ...rest } = baselines;
    return { baselines: baseline ? { ...rest, [id]: baseline } : rest };
  });
}

/** The card pressed Save on its file and waits for the index to read it: read-only until then (9.7). */
export function setSettling(card: string, settling: boolean): void {
  useDrafts.setState(({ settling: held }) => {
    if (held.has(card) === settling) return {};
    const next = new Set(held);
    if (settling) next.add(card);
    else next.delete(card);
    return { settling: next };
  });
}
