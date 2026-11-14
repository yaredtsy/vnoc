// What a screen reader says of a card (docs/16-ui-redesign/09-states.md §6): `function checkout, selected, 1 doc, cursor here`, and of a call card `call to
// function reserve_stock, called twice, lines 16 and 31, collapsed`. Built from the same facts the look is, in one order for a card and a row. Pure.

import { docsCount } from "./footerChips.ts";

/** `once`, `twice`, `3 times`. */
export const timesPhrase = (n: number): string => (n === 1 ? "once" : n === 2 ? "twice" : `${n} times`);

/** `line 16`, `lines 16 and 31`, `lines 16, 31 and 40`: each line once, in order. */
export function linesPhrase(lines: readonly number[]): string {
  const unique = [...new Set(lines)];
  if (unique.length === 1) return `line ${unique[0]}`;
  return `lines ${unique.slice(0, -1).join(", ")} and ${unique[unique.length - 1]}`;
}

export interface CardFacts {
  role: "node" | "call";
  kind: string;
  name: string;
  docs: number;
  /** a call card: the 1-based lines where the parent calls it (one per site) */
  lines: readonly number[];
  /** the callee is not in the project's index */
  unknown: boolean;
  /** it has something to open (children, or calls) */
  canOpen: boolean;
  collapsed: boolean;
  syntaxError: boolean;
}

/**
 * The order is the same for a card and a row (09 §6): what it is, `selected`, what a call card adds (how often and where it is called), the docs, `cursor here`,
 * and for a card that opens, `collapsed` or `expanded`. `aria-selected` is on the element too.
 */
export function cardLabel(c: CardFacts, state: { selected: boolean; cursor: boolean }): string {
  const parts = [`${c.role === "call" ? "call to " : ""}${c.kind} ${c.name}`];
  if (state.selected) parts.push("selected");
  if (c.syntaxError) parts.push("syntax error");
  if (c.role === "call") {
    if (c.unknown) parts.push("not in the index");
    if (c.lines.length > 0) parts.push(`called ${timesPhrase(c.lines.length)}`, linesPhrase(c.lines));
  }
  if (c.docs > 0) parts.push(docsCount(c.docs));
  if (state.cursor) parts.push("cursor here");
  if (c.canOpen) parts.push(c.collapsed ? "collapsed" : "expanded");
  return parts.join(", ");
}
