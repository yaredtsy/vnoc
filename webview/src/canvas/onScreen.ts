// Which cards are on screen (docs/16-ui-redesign/03-canvas.md §6.1): the viewport and one screen of margin on every side. Pure, so the rule is tested;
// the canvas reports the result to the host after a pan settles, and only those cards are probed.

import { CARD_W } from "./layout.ts";

export interface Box {
  id: string;
  x: number;
  y: number;
  h: number;
}

/** React Flow's viewport: where the flow's origin is on screen, and the zoom. */
export interface View {
  x: number;
  y: number;
  zoom: number;
}

/** How long a pan or zoom must stay still before the page says what is on screen. */
export const SETTLE_MS = 300;

/**
 * The ids of the cards that touch the viewport grown by one screen on every side, in the order given. A canvas not measured yet (no size) has no
 * viewport to speak of: every card is on screen then (a drawing is at most MAX_CARDS).
 */
export function onScreenIds(cards: readonly Box[], view: View, size: { width: number; height: number }): string[] {
  if (size.width <= 0 || size.height <= 0 || view.zoom <= 0) return cards.map((c) => c.id);
  const w = size.width / view.zoom;
  const h = size.height / view.zoom;
  const left = -view.x / view.zoom - w;
  const top = -view.y / view.zoom - h;
  const right = -view.x / view.zoom + 2 * w;
  const bottom = -view.y / view.zoom + 2 * h;
  return cards.filter((c) => c.x < right && c.x + CARD_W > left && c.y < bottom && c.y + c.h > top).map((c) => c.id);
}
