// The keys on the canvas (docs/10-ui-rework/06-how-it-looks.md §7). `canvasKey` says what a key means; the page dispatches the intent. Pure.

import type { Intent } from "@vnoc/shared/session.ts";
import type { TreeNode } from "@vnoc/shared/messages.ts";
import { pathTo } from "@vnoc/shared/tree.ts";
import { type Layout, canOpen } from "./layout.ts";

/** The intent for a key, or null when the key means nothing here. `laid.nodes` is in row order, top to bottom. */
export function canvasKey(key: string, laid: Layout, root: TreeNode, selected: string | null): Intent | null {
  const ids = laid.nodes.map((p) => p.id);
  const at = selected ? ids.indexOf(selected) : -1;
  const card = at >= 0 ? laid.nodes[at] : null;
  switch (key) {
    case "ArrowDown":
      return ids.length > 0 ? { type: "select", id: ids[Math.min(at + 1, ids.length - 1)] } : null;
    case "ArrowUp":
      return ids.length > 0 ? { type: "select", id: ids[at <= 0 ? 0 : at - 1] } : null;
    case "ArrowLeft": {
      const way = selected ? pathTo(root, selected) : null;
      const parent = way && way.length > 1 ? way[way.length - 2].id : null;
      return parent && ids.includes(parent) ? { type: "select", id: parent } : null;
    }
    case "ArrowRight": // open it if it is closed, else go to its first child
      if (!card || !canOpen(card)) return null;
      if (card.collapsed) return { type: "expand", ids: [card.id] };
      // open: its first child, when one is drawn right after it (a function open with its calls not drawn has none)
      return laid.nodes[at + 1] && laid.nodes[at + 1].depth > card.depth ? { type: "select", id: ids[at + 1] } : null;
    case "Enter":
      return card ? { type: "focus", id: card.id } : null;
    case " ":
      return card && canOpen(card) ? { type: "toggle", id: card.id } : null;
    case "Escape":
      return selected ? { type: "select", id: null } : null;
    default:
      return null;
  }
}
