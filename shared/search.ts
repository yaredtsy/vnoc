// What the search box and the palette's search share (docs/20-search/05-on-screen.md): where a result is, and which answer to draw. Pure, so the extension and the webview
// use the same code and it is tested without either.

/** Where a result is, for its dim second part: the file, with the 1-based line of the definition for a class or function. */
export const placeOf = (row: { path: string; line: number | null }): string => (row.line === null ? row.path : `${row.path}:${row.line + 1}`);

/** The numbering of one asker's requests: see `newest`. */
export interface Numbering {
  /** The number of the request about to be sent. */
  next(): number;
  /** Whether the answer to request `n` is still the one to draw: no later request was sent, and `drop` was not called since. */
  isNewest(n: number): boolean;
  /** Nothing on its way counts any more (the box was emptied): an answer that arrives later is not drawn. */
  drop(): void;
}

/**
 * Each request gets the next number, and only the answer to the newest is drawn (Q8: nothing is cancelled, so answers to older requests do arrive, in any order).
 * `drop` is for an emptied box. The asker keeps one of these and no counter of its own.
 */
export function newest(): Numbering {
  let last = 0;
  let dropped = 0;
  return {
    next: () => ++last,
    isNewest: (n) => n === last && n > dropped,
    drop: () => {
      dropped = last;
    },
  };
}
