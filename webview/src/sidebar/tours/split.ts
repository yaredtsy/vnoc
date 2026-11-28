// The splitter between the tree and the TOURS section (docs/26-tours/05-on-screen.md §5, "As built (TR18)": the section is 45% of the view when open; TR20 makes it draggable).
// The arithmetic of the drag, pure, so the limits are tests. The section's size is a share of the view's height, kept per window like the panels' (06 §4.1).

import { MIN_PANEL } from "../splitters.ts";

/** The share of the view's height the section has until it is dragged. */
export const DEFAULT_TOURS_SHARE = 0.45;

/** The section's height in a view `total` px high: its share, but never under MIN_PANEL, and never so tall that the tree above it is under MIN_PANEL (then the view scrolls: the tree keeps the minimum). */
export function toursHeight(total: number, share: number): number {
  const most = Math.max(MIN_PANEL, total - MIN_PANEL);
  return Math.min(most, Math.max(MIN_PANEL, Math.round(share * total)));
}

/** The height after a drag of `dy` px (down is positive: the section gets smaller) from a section of `from` px in a view of `total` px. */
export function dragTours(total: number, from: number, dy: number): number {
  return toursHeight(total, (from - dy) / total);
}

/** The share to keep for a height. */
export const toursShare = (total: number, height: number): number => (total > 0 ? height / total : DEFAULT_TOURS_SHARE);
