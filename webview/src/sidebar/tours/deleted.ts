// A tour that was deleted, in the list (docs/26-tours/design/07-tour-list.md §7): for six seconds its row is the core's words and **Undo**, in the place the row had; then it goes. Pure: what the list
// draws from the rows the core gave and the deleted ones the host told of (`tourDeleted`); and the little store that holds them. The sentence is the core's (`words`): nothing is said here.

import { create } from "zustand";

import type { TourRow } from "@vnoc/shared/tours.ts";

/** How long a deleted row stays, with its Undo. */
export const UNDO_MS = 6000;

/** What a delete (or **Delete Finished Tours**: all in one) left in the list. `after` is the id of the row that was above it, or null when it was the first. */
export interface Tomb {
  key: string;
  ids: string[];
  tokens: string[];
  words: string;
  after: string | null;
}

export type Entry = { type: "row"; row: TourRow } | { type: "deleted"; tomb: Tomb };

/** The row above the first of `ids` that is not one of them: where the deleted row goes back to be drawn. null: it was at the top. */
export function afterOf(rows: readonly TourRow[], ids: readonly string[]): string | null {
  const first = rows.findIndex((r) => ids.includes(r.id));
  if (first < 0) return null;
  for (let i = first - 1; i >= 0; i--) if (!ids.includes(rows[i].id)) return rows[i].id;
  return null;
}

/**
 * The entries the list draws: the core's rows, each deleted one's row replaced by one entry for them all, just under the row that was above it (or at the top when that row is gone too).
 * A row that is in `tombs` and still in the core's list (the list was not asked again yet) is not drawn.
 */
export function withTombs(rows: readonly TourRow[], tombs: readonly Tomb[]): Entry[] {
  const hidden = new Set(tombs.flatMap((t) => t.ids));
  const entries: Entry[] = rows.filter((r) => !hidden.has(r.id)).map((row) => ({ type: "row", row }));
  for (const tomb of tombs) {
    const at = tomb.after === null ? -1 : entries.findIndex((e) => e.type === "row" && e.row.id === tomb.after);
    entries.splice(at + 1, 0, { type: "deleted", tomb });
  }
  return entries;
}

export const useDeleted = create<{ tombs: Tomb[] }>(() => ({ tombs: [] }));

/** A tour went to the trash: its row says so for six seconds. */
export function addTomb(tomb: Omit<Tomb, "key">, ms = UNDO_MS): void {
  const key = `${tomb.tokens.join("+")}`;
  useDeleted.setState((s) => ({ tombs: [...s.tombs.filter((t) => t.key !== key), { ...tomb, key }] }));
  setTimeout(() => removeTomb(key), ms);
}

export function removeTomb(key: string): void {
  useDeleted.setState((s) => (s.tombs.some((t) => t.key === key) ? { tombs: s.tombs.filter((t) => t.key !== key) } : s));
}
