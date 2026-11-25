// A tour's row in the list (docs/26-tours/design/07-tour-list.md §3), as what it draws. Pure: the row's own words, as the core gave them, and nothing made here: the
// state word, the size and `shared ·` are all in `words` (the core builds them, `view.rs`), the title and the question are the tour's.

import type { TourRow } from "@vnoc/shared/tours.ts";

export interface ListItem {
  /** unique in the list: the same id in both folders is two rows */
  key: string;
  id: string;
  /** line 1: the tour's title (its id when it cannot be read) */
  title: string;
  /** line 2, the core's `words`, unchanged; a tour that cannot be read says why after it */
  words: string;
  /** the question the tour answers, drawn after the words; null when it has none */
  question: string | null;
  /** the first stop's kind, for the icon; null when the tour has no stop yet */
  kind: string | null;
  /** a tour that cannot be read opens nothing (and has only Delete, in TR22) */
  clickable: boolean;
}

export function listItem(row: TourRow): ListItem {
  return {
    key: `${row.id}:${row.folder}`,
    id: row.id,
    title: row.title ?? row.id,
    words: row.readable ? row.words : row.problem ? `${row.words}: ${row.problem}` : row.words,
    question: row.readable ? (row.question ?? null) : null,
    kind: row.root?.kind ?? null,
    clickable: row.readable,
  };
}

/** The row that is the open tour's: it has the bar of the active panel. The private copy of a shared tour is not the open one. */
export const isOpen = (row: TourRow, open: string | null): boolean => open !== null && row.id === open && !row.private_copy;
