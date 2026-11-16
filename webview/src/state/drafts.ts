// The drafts (docs/17-edit-in-canvas.md E6): one function's unsaved edit each, by the function's key. The drafts are saved with the view, by `view.ts`'s one
// subscriber (`vscode.setState` keeps one object per webview: a second call would wipe the other, measured in step 9.0), so they survive hiding the tab and a
// reload of the webviews. `refused` is not saved: the cards whose Edit the host refused because the file has unsaved changes (E5).
// The rules are `draftRules.ts`; this only holds the result. The things that talk to the host are `edit.ts`.

import { create } from "zustand";

import { vscode } from "../host.ts";
import { type Draft, type Drafts, restoreDrafts, typed, without, withDraft } from "./draftRules.ts";

export const useDrafts = create<{ drafts: Drafts; refused: ReadonlySet<string> }>(() => ({
  drafts: restoreDrafts(vscode.getState<{ drafts?: unknown }>()?.drafts),
  refused: new Set(),
}));

export const putDraft = (d: Draft) => useDrafts.setState(({ drafts }) => ({ drafts: withDraft(drafts, d) }));
export const typeInto = (key: string, text: string) => useDrafts.setState(({ drafts }) => ({ drafts: typed(drafts, key, text) }));
export const dropDraft = (key: string) => useDrafts.setState(({ drafts }) => ({ drafts: without(drafts, key) }));

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
