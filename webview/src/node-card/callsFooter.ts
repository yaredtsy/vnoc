// What the footer says about a card's calls (docs/16-ui-redesign/04-node-cards.md §1, 05 §3.1, §4). Pure, so the table is tested; layout.ts decides it per card.

import type { Calls } from "@vnoc/shared/calls.ts";

export type CallsFooter =
  /** it calls no function in this project */
  | { state: "nothing" }
  /** a probe or its tree says opening shows `n` call cards (and it is closed) */
  | { state: "count"; n: number }
  /** the host is fetching its tree */
  | { state: "loading" }
  /** the fetch failed: Retry */
  | { state: "failed"; message: string }
  /** nothing to say (not known yet, or open: the calls are drawn) */
  | null;

/** The footer's word about calls, from what the canvas knows of them (`Calls`) and whether the card is open. */
export function callsFooter(calls: Calls, open: boolean): CallsFooter {
  switch (calls.state) {
    case "unknown":
      return null;
    case "loading":
      return { state: "loading" };
    case "failed":
      return { state: "failed", message: calls.message };
    case "probed":
      return count(calls.count, open);
    case "known":
      return count(calls.children.length, open);
  }
}

const count = (n: number, open: boolean): CallsFooter => (n === 0 ? { state: "nothing" } : open ? null : { state: "count", n });
