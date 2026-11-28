// The TOURS section's second view, "On the selected node" (docs/26-tours/design/07-tour-list.md §4, §6, §8), and the two small facts of the same feature that sit on a node:
// `▶ N` and `+N`. Pure: the words are the core's where the core has them (`on.words` is line 2 of a row), and what is made here is the buttons' text, which row opens where, and
// the tooltips. Nothing is counted: every number comes from the core (`vnoc/tours`, `vnoc/tourCounts`, `vnoc/tourCalls`).

import { type CallFacts, type CallId, isCallId } from "@vnoc/shared/calls.ts";
import type { TreeNode } from "@vnoc/shared/messages.ts";
import type { Session } from "@vnoc/shared/session.ts";
import { findNode } from "@vnoc/shared/tree.ts";
import type { StopId, TourRow } from "@vnoc/shared/tours.ts";

/** Which list the section draws: the saved choice, unless there is no selection to follow (then it is **All**: the button for the other is disabled). */
export type ListView = "all" | "on";

export function viewOf(saved: Session["tourList"]["view"], selection: string | null): ListView {
  return saved === "onSelection" && selection !== null ? "on" : "all";
}

/**
 * The structure key of the node the list follows: the **active context's** selection, a selected call being its callee (as the docs panel reads it). Nothing selected, a
 * selection the tree does not have, a call whose callee the project does not know, the project's root (every tour passes through it) and a tour context (the section shows
 * that tour's route there) give none.
 */
export function selectionOf(session: Pick<Session, "contexts" | "active">, tree: TreeNode | null, facts: (call: CallId) => CallFacts | null): string | null {
  const c = session.contexts[session.active];
  if (!c || !tree || c.kind === "tour" || c.selected === null) return null;
  const key = isCallId(c.selected) ? (facts(c.selected)?.callee.key ?? null) : c.selected;
  if (key === null || key === tree.id) return null;
  return findNode(tree, key) ? key : null;
}

export interface ViewButton {
  id: ListView;
  /** `All 5`, `On reserve_stock 3`; without a selection `On selection` */
  text: string;
  disabled: boolean;
  tip: string | null;
  pressed: boolean;
}

/** The two buttons: the counts are the lengths of the two lists the core gave. `onCount` is null until the core has answered for the selection. */
export function viewButtons(view: ListView, all: number, selection: { name: string } | null, onCount: number | null): ViewButton[] {
  return [
    { id: "all", text: `All ${all}`, disabled: false, tip: null, pressed: view === "all" },
    {
      id: "on",
      text: selection === null ? "On selection" : onCount === null ? `On ${selection.name}` : `On ${selection.name} ${onCount}`,
      disabled: selection === null,
      tip: selection === null ? "Select a node to see its tours" : null,
      pressed: view === "on",
    },
  ];
}

/** `←` and `→` on the two buttons switch the view (design 07 §8); a key that goes nowhere (the other button is disabled, or it is the one chosen) says nothing. */
export function viewKey(key: string, view: ListView, canOn: boolean): ListView | null {
  if (key === "ArrowLeft" && view === "on") return "all";
  if (key === "ArrowRight" && view === "all" && canOn) return "on";
  return null;
}

/** Line 2 of a row: in the second view, the core's account of how the tour is on the node (`on.words`); in **All**, the row's own words. */
export const lineTwo = (row: TourRow, view: ListView): string => (view === "on" && row.on ? row.on.words : row.words);

/** The word at the right of line 1: in **On the selected node** the row keeps its state (line 2 is then how the tour is on the node); **All** has it in line 2 already. */
export const stateWordOf = (row: TourRow, view: ListView): string | null => (view === "on" && row.state_word ? row.state_word : null);

/**
 * Where a click on a row opens its tour: in **On the selected node**, at the cover for a tour that starts at the node, else at the stop the core says (`on.first`: the first of
 * the stops that are the node, or of those inside it). `null`: where the reader left it (**All**).
 */
export function opensAt(row: TourRow, view: ListView): "cover" | StopId | null {
  if (view !== "on" || !row.on) return null;
  return row.on.how === "starts" ? "cover" : row.on.first;
}

/** The empty state of the second view (design 07 §8). */
export const emptyOn = (name: string): string => `No tour goes through ${name}.`;
export const showAll = (n: number): string => `Show All ${n}`;
/** The empty state's buttons that ask for a tour (design 07 §8): **Ask for One** where the second view is empty; where there is no tour at all **Copy a Request for `name`** (the selection), or **Copy a Request**. Each copies the core's sentence. */
export const ASK_FOR_ONE = "Ask for One";
export const copyRequest = (name: string | null): string => (name === null ? "Copy a Request" : `Copy a Request for ${name}`);

// ── on a node ──

/** `▶ 3`: the node is on 3 tours. */
export const toursChipText = (n: number): string => `▶ ${n}`;
export const toursChipTip = (n: number): string => (n === 1 ? "on 1 tour: click to see it" : `on ${n} tours: click to see them`);

/** The card's `+N`: the node's calls that are not on this tour. A fact, not a button. */
export const offChipText = (n: number): string => `+${n}`;
export const offChipTip = (n: number): string => (n === 1 ? "1 call is not on this tour" : `${n} calls are not on this tour`);
