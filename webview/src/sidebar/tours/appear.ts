// When the TOURS section unfolds by itself (docs/26-tours/design/07-tour-list.md §2): a tour appeared from outside (the agent started one) while this window has none open.
// A tour the reader opened unfolds it already, and a list that was only asked again has no new tour in it. Pure.

/** `before` is the ids of the last list this page held (null: none yet, so the first list is not "new"), `now` the ids of the list that arrived, `open` the tour this window has open. */
export function appearedFromOutside(before: readonly string[] | null, now: readonly string[], open: string | null): boolean {
  if (before === null || open !== null) return false;
  const known = new Set(before);
  return now.some((id) => !known.has(id));
}
