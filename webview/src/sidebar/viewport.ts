// Whether a row is in the visible part of the list (the sidebar's `rendered` note says so, and the test suite reads it). Pure.

/**
 * The `scrollTop` that brings a row into view with the least movement: the same when it already is, the row's top when it is above, its bottom at the
 * area's bottom when it is below. Rows are all one height, so this is arithmetic: no measuring, and nothing that retries on a later frame.
 */
export function scrollFor(index: number, scrollTop: number, height: number, rowHeight: number): number {
  if (index < 0 || height <= 0) return scrollTop;
  const top = index * rowHeight;
  if (top < scrollTop) return top;
  if (top + rowHeight > scrollTop + height) return top + rowHeight - height;
  return scrollTop;
}

/** A row is in view when all of it is inside the scroll area. `index`: the row's place in the list; every row is `rowHeight` high. */
export function inView(index: number, scrollTop: number, height: number, rowHeight: number): boolean {
  if (index < 0) return false;
  const top = index * rowHeight;
  return top >= scrollTop && top + rowHeight <= scrollTop + height;
}
