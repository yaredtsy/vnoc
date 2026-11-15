// What the footer says about a card's calls (docs/16-ui-redesign/04-node-cards.md §1, 05 §3.1, §4). Pure, so the table is tested; layout.ts decides it per card.

import type { Calls } from "@vnoc/shared/calls.ts";

export type CallsFooter =
  /** it calls no function in this project */
  | { state: "nothing" }
  /** the host is fetching its tree */
  | { state: "loading" }
  /** the fetch failed: Retry */
  | { state: "failed"; message: string }
  /** nothing to say: not known yet, or it has calls (a closed card's `+N` counts them, an open one draws them) */
  | null;

/** The footer's word about calls, from what the canvas knows of them (`Calls`). There is no `calls 4`: the `+N` of a closed card already counts them (04 §1). */
export function callsFooter(calls: Calls): CallsFooter {
  switch (calls.state) {
    case "unknown":
      return null;
    case "loading":
      return { state: "loading" };
    case "failed":
      return { state: "failed", message: calls.message };
    case "probed":
      return count(calls.count);
    case "known":
      return count(calls.children.length);
  }
}

const count = (n: number): CallsFooter => (n === 0 ? { state: "nothing" } : null);
