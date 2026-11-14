// What the card's footer counts (docs/16-ui-redesign/04-node-cards.md §1). Pure, so the rules are tested; NodeFooter only draws them.

import type { CallInfo, Placed } from "../canvas/layout.ts";

/** What the footer reads of a card: the counts, and for a call card how many calls were merged and where. */
export type FooterNode = Pick<Placed, "collapsed" | "plus" | "docs" | "link"> & { call: Pick<CallInfo, "count" | "callerName" | "lines"> | null };

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

/** "line 16", "lines 16, 31": the 1-based lines of a call's sites, each once, in order. */
const linesText = (lines: readonly number[]): string => {
  const unique = [...new Set(lines)];
  return `${unique.length === 1 ? "line" : "lines"} ${unique.join(", ")}`;
};

/** The chips in the footer's order: `×N` (a call node that merges several calls), what a closed card hides, docs, the link, the marks. `hides`: whether this card, closed, holds the selection or the text cursor inside it. */
export function footerChips(node: FooterNode, hides: { selected: boolean; cursor: boolean }): Chip[] {
  const chips: Chip[] = [];
  const merged = node.call;
  // `×N` says "N calls from this parent, at these lines". The core's count is the calls that reach the node, which under a parent that was itself merged is
  // more than the places it is written (two calls to `reserve_stock` each call `_lookup` once: count 2, one site): there the tooltip would name one line
  // for "2 times", so the chip is shown only where the sites back the count.
  if (merged && merged.count > 1 && merged.lines.length === merged.count) chips.push({ kind: "times", text: `×${merged.count}`, tip: `${merged.callerName} calls it ${merged.count} times: ${linesText(merged.lines)}`, faded: false });
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
