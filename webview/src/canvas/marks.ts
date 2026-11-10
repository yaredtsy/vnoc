// Where the selection and the cursor are, for the parts of the canvas that point at them. Pure, so it is tested without a browser.

import type { TreeNode } from "@vnoc/shared/messages.ts";
import { pathTo } from "@vnoc/shared/tree.ts";

/** The card that holds `id` when `id` itself is not drawn (a closed card hides it): its nearest drawn ancestor. `null` when `id` is drawn, or not in the tree. */
export function holderOf(root: TreeNode, drawn: ReadonlySet<string>, id: string | null): string | null {
  if (!id || drawn.has(id)) return null;
  const way = pathTo(root, id);
  if (!way) return null;
  for (let i = way.length - 2; i >= 0; i--) if (drawn.has(way[i].id)) return way[i].id;
  return null;
}

/** The edges (ids as the layout makes them) on the way from the root of what is drawn down to `id`. */
export function pathEdges(root: TreeNode, drawnRoot: string, id: string | null): Set<string> {
  const way = id ? pathTo(root, id) : null;
  const edges = new Set<string>();
  if (!way) return edges;
  const from = way.findIndex((n) => n.id === drawnRoot);
  if (from < 0) return edges;
  for (let i = from; i < way.length - 1; i++) edges.add(`${way[i].id}->${way[i + 1].id}`);
  return edges;
}
