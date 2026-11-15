// What a card's chevron, `Space` and `→` do (docs/16-ui-redesign/03-canvas.md §6.1, "a held-back card"). Pure, so the table is tested; the header, the
// footer's `+N` and the keys only follow it.

import type { Placed } from "../canvas/layout.ts";

export type ChevronAction = "focus" | "explore" | "toggle";

/**
 * A card the budget drew closed although it is open in the set (`heldBack`) is not toggled: that would take it out of the set with nothing changing on
 * screen. A structure card is focused, a call card explored: it becomes the root of a view, whose whole family is drawn. Any other card toggles.
 */
export const chevronAction = (card: Pick<Placed, "role" | "heldBack">): ChevronAction => (!card.heldBack ? "toggle" : card.role === "call" ? "explore" : "focus");

/** The tooltip of a held-back card's `+N` (03 §6.1). */
export const heldBackTip = (card: Pick<Placed, "role">): string =>
  `Not drawn: the canvas shows at most 200 cards. Click to ${card.role === "call" ? "explore" : "focus on"} it.`;
