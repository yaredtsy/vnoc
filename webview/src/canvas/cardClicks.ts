// What a double-click on a card does (docs/16-ui-redesign/04-node-cards.md §4). Pure, so the table is tested; the canvas only dispatches it.

import type { Intent } from "@vnoc/shared/session.ts";
import type { Placed } from "./layout.ts";

/** A structure card: Focus. A call card whose callee the project knows: Explore, activated (it follows the call). An unknown callee: only a selection. */
export function doubleClickIntent(card: Pick<Placed, "id" | "role" | "key">): Intent {
  if (card.role === "node") return { type: "focus", id: card.id };
  return card.key === null ? { type: "select", id: card.id } : { type: "explore", call: card.id, activate: true };
}
