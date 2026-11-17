// The search box (docs/16-ui-redesign/06-sidebar.md §5, docs/20-search/05-on-screen.md §4): what its keys mean, what a pick does, and a name cut around the
// letters to draw bold. Pure, so the rules are tests; the page draws. What matches, and in what order, is the core's (`vnoc/search`): nothing here ranks.

import type { SearchNode } from "@vnoc/shared/messages.ts";
import type { Intent } from "@vnoc/shared/session.ts";

/** What a key means in the box while the results are shown: the active result moves, the active one is picked, or the box clears. */
export type SearchKey = { type: "move"; to: number } | { type: "pick" } | { type: "clear" };

/**
 * `↓` from the box and `↑` `↓` in the list move the active result (they stop at the ends); `Enter` picks it; `Esc` clears the box. The focus stays in
 * the box throughout (the list is its `aria-activedescendant`), so typing carries on. Anything else is the box's own (typing, the caret).
 */
export function searchKey(key: string, count: number, active: number): SearchKey | null {
  switch (key) {
    case "ArrowDown":
      return count > 0 ? { type: "move", to: Math.min(active + 1, count - 1) } : null;
    case "ArrowUp":
      return count > 0 ? { type: "move", to: Math.max(active - 1, 0) } : null;
    case "Enter":
      return count > 0 ? { type: "pick" } : null;
    case "Escape":
      return { type: "clear" };
    default:
      return null;
  }
}

/**
 * What a pick does: the node is revealed in the active context (03 §3): the focus shortens until it holds the node, the ancestors between open, the node is
 * selected. The click rule of 07 §4 follows (`rowClick`), as the palette's search does.
 */
export const pickIntent = (id: string): Intent => ({ type: "reveal", id, canvas: false });

/** A part of a name, and whether it is drawn bold. */
export interface Piece {
  text: string;
  bold: boolean;
}

/**
 * A name cut around the ranges the core found in it (`match` of a `SearchNode`), in order. The ranges are UTF-16 offsets, which is what `slice` takes, so no
 * conversion is needed. Two ranges that touch give two bold pieces side by side. A part with no text is left out; a name with no range is one piece.
 */
export function splitMatch(name: string, ranges: readonly [number, number][]): Piece[] {
  const pieces: Piece[] = [];
  let at = 0;
  for (const [from, to] of ranges) {
    const start = Math.max(from, at); // a range that overlaps the last one is cut to what is left (the core merges them: this only keeps the pieces in order)
    const end = Math.min(to, name.length);
    if (end <= start) continue;
    if (start > at) pieces.push({ text: name.slice(at, start), bold: false });
    pieces.push({ text: name.slice(start, end), bold: true });
    at = end;
  }
  if (at < name.length) pieces.push({ text: name.slice(at), bold: false });
  return pieces;
}

/** Where a result is, for its dim second part: the file, with the 1-based line of the definition for a class or function. */
export const placeOf = (row: Pick<SearchNode, "path" | "line">): string => (row.line === null ? row.path : `${row.path}:${row.line + 1}`);

/** The header over the rows: `Nodes 12`, or `Nodes 200 of 1,340` when the limit cut them. */
export const nodesHeader = (shown: number, total: number): string => (total > shown ? `Nodes ${shown.toLocaleString("en-US")} of ${total.toLocaleString("en-US")}` : `Nodes ${shown.toLocaleString("en-US")}`);
