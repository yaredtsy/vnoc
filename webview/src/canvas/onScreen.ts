// Which cards are on screen (docs/16-ui-redesign/03-canvas.md §6.1): the viewport and one screen of margin on every side. Pure, so the rule is tested;
// the canvas reports the result to the host after a pan settles, and only those cards are probed.

import { type Placed } from "./layout.ts";

export interface Box {
  id: string;
  x: number;
  y: number;
  /** the card's width (it is wider with its code open) */
  w: number;
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
  return cards.filter((c) => c.x < right && c.x + c.w > left && c.y < bottom && c.y + c.h > top).map((c) => c.id);
}

/**
 * What the page tells the host about the cards on screen (`visible`: their ids). `onScreen`: every function and class card there, whatever is under it: the
 * prefetch of the selected one is checked against these. `keys`: those with no structure children, which show nothing about their calls until probed; the
 * selected one first. A class with methods is in `onScreen` and not in `keys`: it owns a tree when it has no `__init__`, and its prefetch must survive.
 */
export function cardsReport(cards: readonly Pick<Placed, "id" | "role" | "kind" | "childCount">[], visible: ReadonlySet<string>, selected: string | null): { keys: string[]; onScreen: string[] } {
  const shown = cards.filter((p) => p.role === "node" && (p.kind === "function" || p.kind === "class") && visible.has(p.id));
  const keys = shown.filter((p) => p.childCount === 0).map((p) => p.id);
  return { keys: selected !== null && keys.includes(selected) ? [selected, ...keys.filter((k) => k !== selected)] : keys, onScreen: shown.map((p) => p.id) };
}
