// The search box (docs/16-ui-redesign/06-sidebar.md §5): what matches, in what order, and what its keys mean. Pure, so the rules are tests; the page draws.

import type { TreeNode } from "@vnoc/shared/messages.ts";
import type { Intent } from "@vnoc/shared/session.ts";

/** At most this many results are shown, best first. */
export const RESULT_LIMIT = 200;

/** A node of the project with what a result needs of its place: a function inside a class gets a method's icon. */
export interface Indexed {
  node: TreeNode;
  inClass: boolean;
}

/** Every node of the project in reading order, but the opened folder itself (it is not a node of the project). */
export function indexNodes(root: TreeNode): Indexed[] {
  const out: Indexed[] = [];
  const stack: { node: TreeNode; inClass: boolean }[] = [];
  for (let i = root.children.length - 1; i >= 0; i--) stack.push({ node: root.children[i], inClass: false });
  while (stack.length > 0) {
    const item = stack.pop()!;
    out.push(item);
    const kids = item.node.children;
    for (let i = kids.length - 1; i >= 0; i--) stack.push({ node: kids[i], inClass: kids[i].kind === "function" && item.node.kind === "class" });
  }
  return out;
}

/** How a result matched. A smaller tier is better: the name before the path, a prefix before a substring. */
export type Tier = 0 | 1 | 2 | 3;

export interface Result extends Indexed {
  tier: Tier;
  /** the part of the name that matched (to draw bold); null when only the path did */
  match: [start: number, end: number] | null;
}

/** Where `query` is in `name`, ignoring case: the letters to draw bold. */
export function matchRange(name: string, query: string): [number, number] | null {
  const q = query.trim().toLowerCase();
  const at = q === "" ? -1 : name.toLowerCase().indexOf(q);
  return at < 0 ? null : [at, at + q.length];
}

/**
 * The nodes whose name or path hold `query` (case does not matter), best first: name starts with it, name holds it, path starts with it (`billing/` finds what is
 * under that folder), path holds it. Within a tier the shorter name first (the closer to the whole name), then reading order. At most `limit`. An empty query
 * finds nothing.
 */
export function rankNodes(index: readonly Indexed[], query: string, limit: number = RESULT_LIMIT): Result[] {
  const q = query.trim().toLowerCase();
  if (q === "") return [];
  const found: (Result & { at: number })[] = [];
  index.forEach((entry, at) => {
    const name = entry.node.name.toLowerCase();
    const path = entry.node.path.toLowerCase();
    const tier: Tier | null = name.startsWith(q) ? 0 : name.includes(q) ? 1 : path.startsWith(q) ? 2 : path.includes(q) ? 3 : null;
    if (tier !== null) found.push({ ...entry, tier, match: tier <= 1 ? matchRange(entry.node.name, q) : null, at });
  });
  found.sort((a, b) => a.tier - b.tier || a.node.name.length - b.node.name.length || a.at - b.at);
  return found.slice(0, limit).map(({ at: _at, ...result }) => result);
}

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

/** A name as three parts around its match, to draw the middle bold. */
export function splitMatch(name: string, match: [number, number] | null): [string, string, string] {
  return match ? [name.slice(0, match[0]), name.slice(match[0], match[1]), name.slice(match[1])] : [name, "", ""];
}
