// Whether a row is in the visible part of the list (the sidebar's `rendered` note says so, and the test suite reads it). Pure.

/** A row is in view when all of it is inside the scroll area. `index`: the row's place in the list; every row is `rowHeight` high. */
export function inView(index: number, scrollTop: number, height: number, rowHeight: number): boolean {
  if (index < 0) return false;
  const top = index * rowHeight;
  return top >= scrollTop && top + rowHeight <= scrollTop + height;
}
