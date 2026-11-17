// Which cards the canvas dims while the sidebar's box has text (docs/20-search/05-on-screen.md §3). What the query finds is the core's (`vnoc/searchCards`);
// this only turns its answer into the cards to draw dim. Pure, so the rules are tests.

/** What the canvas knows of an answer: which keys it was asked about, and which of them the query finds. */
export interface Dim {
  asked: ReadonlySet<string>;
  hits: ReadonlySet<string>;
}

/** The keys to ask about: every laid-out card's, once each, in the order laid out. A call card to something outside the index has none. */
export const keysOf = (cards: readonly { key: string | null }[]): string[] => [...new Set(cards.flatMap((c) => (c.key === null ? [] : [c.key])))];

/**
 * The ids of the cards to dim. A card the query does not find is dimmed, and a call card with no key always is. The selected card never is: it is what the
 * user is looking at. A card whose key was not in the question (it appeared since, and the next answer will cover it) is left as it is, so nothing flashes.
 * No answer yet: nothing is dimmed.
 */
export function dimmedCards(cards: readonly { id: string; key: string | null }[], dim: Dim | null, selected: string | null): string[] {
  if (dim === null) return [];
  return cards.filter((c) => c.id !== selected && (c.key === null ? true : dim.asked.has(c.key) && !dim.hits.has(c.key))).map((c) => c.id);
}
