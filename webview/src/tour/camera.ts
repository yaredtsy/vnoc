// The camera of a tour (docs/26-tours/design/04-route-and-map.md §4): it moves the map as little as it can. The one rule: the current card is in view after every step,
// and nothing else is promised. This is geometry, not tour logic: the card's box, the viewport and the pane's size in, a new viewport or nothing out. Pure.

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Viewport {
  x: number;
  y: number;
  zoom: number;
}

/** The margin kept around a card that is panned into view (design 04 §4). */
export const MARGIN = 24;

/**
 * The viewport to move to so that `card` (in the map's own coordinates) is whole in a pane `size` px big, or null when it is whole in view already.
 * - whole in view (inside the pane): no move;
 * - cut or out of view: the least pan that shows it all with a 24 px margin; the zoom is never touched;
 * - when that pan is more than one screen, or the card (with its margin) is bigger than the pane, the card is centred instead.
 */
export function cameraMove(card: Box, view: Viewport, size: { width: number; height: number }): Viewport | null {
  const left = card.x * view.zoom + view.x;
  const top = card.y * view.zoom + view.y;
  const right = left + card.width * view.zoom;
  const bottom = top + card.height * view.zoom;
  if (left >= 0 && top >= 0 && right <= size.width && bottom <= size.height) return null;
  const pan = (low: number, high: number, extent: number): number => {
    // the least shift that puts [low, high] inside [margin, extent - margin]
    if (low < MARGIN) return MARGIN - low;
    if (high > extent - MARGIN) return extent - MARGIN - high;
    return 0;
  };
  const dx = pan(left, right, size.width);
  const dy = pan(top, bottom, size.height);
  const bigger = right - left + 2 * MARGIN > size.width || bottom - top + 2 * MARGIN > size.height;
  if (bigger || Math.abs(dx) > size.width || Math.abs(dy) > size.height) return centre(card, view.zoom, size);
  return { x: view.x + dx, y: view.y + dy, zoom: view.zoom };
}

/** The viewport that puts the middle of `card` in the middle of the pane at this zoom (`⌖`, and the long way). */
export function centre(card: Box, zoom: number, size: { width: number; height: number }): Viewport {
  return { x: size.width / 2 - (card.x + card.width / 2) * zoom, y: size.height / 2 - (card.y + card.height / 2) * zoom, zoom };
}

/** The bar's `⌖` sends this to the map: centre the current stop's card (the map listens; nothing else changes). */
export const CENTER_TOUR_EVENT = "vnoc:centerTour";
