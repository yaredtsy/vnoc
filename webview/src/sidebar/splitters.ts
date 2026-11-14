// The splitters between the panels (docs/16-ui-redesign/06-sidebar.md §4.1). The arithmetic of heights and drags, pure, so the limits are tests. A panel's
// size is a share of the view's height, kept per depth; MAIN, the first, takes what the others leave.

/** Every panel keeps its header, its focus bar and three rows: about 112 px. */
export const MIN_PANEL = 112;
/** A new EXPLORE panel, and a double-click on its splitter, give it 45% of the view. */
export const DEFAULT_SHARE = 0.45;

/**
 * The heights of the panels in a view `total` px high. `shares` are the shares of every panel after the first (MAIN takes the rest). A panel is never under
 * MIN_PANEL; when MAIN would be, the panels below give up room from the last one up, and when even that is not enough the heights add up to more than `total`
 * and the view scrolls.
 */
export function panelHeights(total: number, shares: readonly number[]): { heights: number[]; scroll: boolean } {
  if (shares.length === 0) return { heights: [total], scroll: false };
  const heights = shares.map((share) => Math.max(MIN_PANEL, Math.round(share * total)));
  let missing = MIN_PANEL - (total - heights.reduce((a, b) => a + b, 0));
  for (let i = heights.length - 1; i >= 0 && missing > 0; i--) {
    const take = Math.min(heights[i] - MIN_PANEL, missing);
    heights[i] -= take;
    missing -= take;
  }
  const main = Math.max(MIN_PANEL, total - heights.reduce((a, b) => a + b, 0));
  const all = [main, ...heights];
  return { heights: all, scroll: all.reduce((a, b) => a + b, 0) > total };
}

/**
 * Drags the splitter on the top edge of panel `index` (1 or more) by `dy` px (down is positive): the panel below changes by `-dy` and the one above takes or
 * gives the room, and a drag stops when either would go under MIN_PANEL; no other panel moves. The same heights when there is nothing to move.
 */
export function dragSplitter(heights: readonly number[], index: number, dy: number): number[] {
  if (index < 1 || index >= heights.length) return [...heights];
  const lo = MIN_PANEL - heights[index - 1]; // the panel above may not go under its minimum
  const hi = heights[index] - MIN_PANEL; // nor the one below
  const d = lo > hi ? 0 : Math.min(Math.max(dy, lo), hi);
  const out = [...heights];
  out[index - 1] += d;
  out[index] -= d;
  return out;
}

/** A double-click on the splitter of panel `index`: the panel below goes back to DEFAULT_SHARE of the view (as far as the limits allow). */
export const resetSplitter = (total: number, heights: readonly number[], index: number): number[] => dragSplitter(heights, index, heights[index] - Math.round(DEFAULT_SHARE * total));

/** The shares to keep, from heights: one per panel after the first. */
export const sharesOf = (total: number, heights: readonly number[]): number[] => (total > 0 ? heights.slice(1).map((h) => h / total) : heights.slice(1).map(() => DEFAULT_SHARE));
