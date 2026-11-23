// A reveal brings one card into view (`CenterOn`), and moves the camera only when it has to (docs/24-notes-film/05-build-order.md §5 P12): a card that is already whole inside the pane, at a zoom a card can be read at,
// is selected and pulses and nothing else moves. Otherwise it is framed (fitView on the one card). The pane may change size a moment later (a pick from the search opens the docs tab beside the canvas, which takes half
// of the canvas's editor group), and the same test is made then. Pure: the test, and the window in which the pane's change is followed.

/** The smallest zoom at which a reveal leaves the camera alone: below it a card cannot be read, and the card is framed even if it is whole. */
export const MIN_READABLE_ZOOM = 0.5;

/** A box in the canvas's own (flow) coordinates, or in the pane's pixels. */
export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The canvas's viewport: where the flow's origin is in the pane (`x`, `y`, in pixels) and the scale. */
export interface View {
  x: number;
  y: number;
  zoom: number;
}

/** Half a pixel: a card that touches the pane's edge exactly is inside it, whatever the last digit of the arithmetic says. */
const EDGE = 0.5;

/**
 * Whether a reveal has to move the camera: the zoom is below `MIN_READABLE_ZOOM`, or the card (`card`, in flow coordinates) is not whole inside the pane (`pane`) at this `view`. A pane of no size is not known:
 * the card is framed.
 */
export function needsFraming(card: Box, pane: { width: number; height: number }, view: View): boolean {
  if (!(pane.width > 0) || !(pane.height > 0) || !(view.zoom > 0)) return true;
  if (view.zoom < MIN_READABLE_ZOOM) return true;
  const left = card.x * view.zoom + view.x;
  const top = card.y * view.zoom + view.y;
  const whole = left >= -EDGE && top >= -EDGE && left + card.width * view.zoom <= pane.width + EDGE && top + card.height * view.zoom <= pane.height + EDGE;
  return !whole;
}

/** How long after a reveal a change of the pane's size is looked at again. The docs tab is up within about 0.3 s of a click; the window is not so long that a person's own resize is taken over. */
export const FOLLOW_MS = 1500;

/** `at`: when the last reveal was judged (null: none was); `now`: the same clock. */
export const followsReveal = (at: number | null, now: number): boolean => at !== null && now - at >= 0 && now - at <= FOLLOW_MS;

/**
 * The card the docs panel just opened for: the panel went from closed to open and a card is selected and drawn. The panel narrows the pane, so the card is judged again once the pane has its new size (the same
 * `needsFraming` and the same `FOLLOW_MS` as a reveal: the panel opening is the change that is followed, a person's own resize of the panel later is not). Null when the panel did not just open, or nothing drawn is selected.
 */
export const docsOpenedFor = (wasOpen: boolean, isOpen: boolean, selected: string | null, isDrawn: (id: string) => boolean): string | null => (!wasOpen && isOpen && selected !== null && isDrawn(selected) ? selected : null);
