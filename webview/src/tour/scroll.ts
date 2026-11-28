// When the code pane scrolls by itself (docs/26-tours/design/03-stage.md §2.2). Pure: lines are counted from 0 in the code the pane shows, `top` is the first one
// in view and `rows` how many are in view.

export interface PaneView {
  top: number;
  rows: number;
}

/** The lit lines of a step in the shown code, from the first to the last, inclusive. */
export type Lit = readonly [number, number];

/**
 * The line to put at the top of the pane after the step changed, or null when the pane stays where it is.
 * - a new stop: the top (the code is another code), unless the lit lines are not in view from the top: then the first lit line goes two lines under the top (a block far
 *   below the function's start, reached by `go`, the route or the map);
 * - a new step in the same stop: only if the lit lines are not fully in view, and then the first lit line goes two lines under the top;
 * - never for a step of the same stop while the pointer is over the pane: it is being read or selected.
 */
export function scrollTarget(a: { newStop: boolean; lit: Lit | null; view: PaneView; pointerOver: boolean }): number | null {
  const { view } = a;
  if (a.newStop) {
    const target = a.lit === null || a.lit[1] < view.rows ? 0 : Math.max(0, a.lit[0] - 2);
    return target === view.top ? null : target;
  }
  if (a.pointerOver || a.lit === null) return null;
  const [first, last] = a.lit;
  if (first >= view.top && last < view.top + view.rows) return null; // fully in view: the code does not move
  const target = Math.max(0, first - 2);
  return target === view.top ? null : target;
}

/** How many lit lines are below the bottom of the pane (`▾ 9 more lit lines`): a block taller than what is in view. 0: the chip is not shown. */
export function moreLit(lit: Lit | null, view: PaneView): number {
  if (lit === null) return 0;
  const bottom = view.top + view.rows - 1;
  return Math.max(0, lit[1] - Math.max(bottom, lit[0] - 1));
}

/** The top after a click on the chip: one pane down, never past the last line. */
export function nextPage(view: PaneView, total: number): number {
  return Math.max(0, Math.min(view.top + view.rows, total - 1));
}

/**
 * What is in view of a pane whose lines are not all one height (a long line is wrapped onto more rows): `tops` and `heights` are those of each line in px from the pane's own top
 * (scrolled with it), `scrollTop` and `clientHeight` the pane's. `top` is the first line more than half in view, `rows` how many lines are whole in view from it (at least 1).
 */
export function viewOf(tops: readonly number[], heights: readonly number[], scrollTop: number, clientHeight: number): PaneView {
  let top = tops.findIndex((t, i) => t + heights[i] / 2 > scrollTop);
  if (top < 0) top = Math.max(0, tops.length - 1);
  let rows = 0;
  for (let i = top; i < tops.length && tops[i] + heights[i] <= scrollTop + clientHeight + 1; i++) rows++;
  return { top, rows: Math.max(1, rows) };
}
