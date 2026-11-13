// What the card's footer counts (docs/16-ui-redesign/04-node-cards.md §1). Pure, so the rules are tested; NodeFooter only draws them.

import type { Placed } from "../canvas/layout.ts";

export interface Chip {
  kind: "hidden" | "docs" | "link" | "selected" | "cursor";
  /** the text beside the icon (the chip's own mark for `selected` and `cursor`); empty when the icon says it all */
  text: string;
  tip: string;
  /** drawn dim: a link that was found by name only */
  faded: boolean;
}

/** "1 doc", "2 docs". */
export const docsCount = (n: number): string => `${n} ${n === 1 ? "doc" : "docs"}`;

/** The chips in the footer's order. `hides`: whether this card, closed, holds the selection or the text cursor inside it. */
export function footerChips(node: Pick<Placed, "collapsed" | "plus" | "docs" | "link">, hides: { selected: boolean; cursor: boolean }): Chip[] {
  const chips: Chip[] = [];
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
