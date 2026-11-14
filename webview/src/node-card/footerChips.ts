// What the card's footer counts (docs/16-ui-redesign/04-node-cards.md §1). Pure, so the rules are tested; NodeFooter only draws them.

import { linesText } from "@vnoc/shared/calls.ts";
import type { CallInfo, Placed } from "../canvas/layout.ts";

/** What the footer reads of a card: the counts, and for a call card who calls it and at which lines. */
export type FooterNode = Pick<Placed, "collapsed" | "plus" | "docs" | "link"> & { call: Pick<CallInfo, "callerName" | "lines"> | null };

export interface Chip {
  kind: "times" | "hidden" | "docs" | "link" | "selected" | "cursor";
  /** the text beside the icon (the chip's own mark for `selected` and `cursor`); empty when the icon says it all */
  text: string;
  tip: string;
  /** drawn dim: a link that was found by name only */
  faded: boolean;
}

/** "1 doc", "2 docs". */
export const docsCount = (n: number): string => `${n} ${n === 1 ? "doc" : "docs"}`;

/** The chips in the footer's order: `×N` (a call node with several call sites), what a closed card hides, docs, the link, the marks. `hides`: whether this card, closed, holds the selection or the text cursor inside it. */
export function footerChips(node: FooterNode, hides: { selected: boolean; cursor: boolean }): Chip[] {
  const chips: Chip[] = [];
  const merged = node.call;
  // `×N` is the places the parent calls it: the call sites on the card. Never the core's `call_count`, which counts the calls that reach the node, once per
  // copy of a merged parent (two calls to `reserve_stock` each call `_lookup` once: count 2, one site).
  if (merged && merged.lines.length > 1) {
    const times = merged.lines.length;
    chips.push({ kind: "times", text: `×${times}`, tip: `${merged.callerName} calls it ${times} times: ${linesText(merged.lines)}`, faded: false });
  }
  if (node.collapsed && node.plus > 0) chips.push({ kind: "hidden", text: `+${node.plus}`, tip: `${node.plus} hidden: open to see them`, faded: false });
  if (node.docs > 0) {
    chips.push({ kind: "docs", text: String(node.docs), tip: `${docsCount(node.docs)}: click to read`, faded: false });
    if (node.link) {
      const byName = node.link === "name";
      chips.push({ kind: "link", text: "", tip: byName ? "found by name only: Write ID to keep it on renames" : "ID in the code", faded: byName });
    }
  }
  if (hides.selected) chips.push({ kind: "selected", text: "●", tip: "the selection is inside", faded: false });
  if (hides.cursor) chips.push({ kind: "cursor", text: "○", tip: "the cursor is inside", faded: false });
  return chips;
}
