// The keys on the canvas (docs/10-ui-rework/06-how-it-looks.md §7). `canvasKey` says what a key means; the page dispatches the intent. Pure.

import { type OpenableSite, listedSites } from "@vnoc/shared/calls.ts";
import type { CallTreeNode } from "@vnoc/shared/messages.ts";
import type { Intent } from "@vnoc/shared/session.ts";
import type { TreeNode } from "@vnoc/shared/messages.ts";
import { chevronAction } from "../node-card/chevronAction.ts";
import { type Layout, type Placed, canOpen } from "./layout.ts";

/** What a toggle of a card is as an intent: a held-back card is focused or explored instead (03 §6.1); `open` is what a plain toggle becomes. */
function chevronIntent(card: Placed, open: Intent): Intent {
  switch (chevronAction(card)) {
    case "focus":
      return { type: "focus", id: card.id };
    case "explore":
      return { type: "explore", call: card.id, activate: true };
    case "toggle":
      return open;
  }
}

/** The intent for a key, or null when the key means nothing here. `laid.nodes` is in row order, top to bottom. */
export function canvasKey(key: string, laid: Layout, _root: TreeNode, selected: string | null): Intent | null {
  const ids = laid.nodes.map((p) => p.id);
  const at = selected ? ids.indexOf(selected) : -1;
  const card = at >= 0 ? laid.nodes[at] : null;
  switch (key) {
    case "ArrowDown":
      return ids.length > 0 ? { type: "select", id: ids[Math.min(at + 1, ids.length - 1)] } : null;
    case "ArrowUp":
      return ids.length > 0 ? { type: "select", id: ids[at <= 0 ? 0 : at - 1] } : null;
    case "ArrowLeft": // the card it hangs from, a call card's too
      return card?.parentId && ids.includes(card.parentId) ? { type: "select", id: card.parentId } : null;
    case "ArrowRight": // open it if it is closed, else go to its first child
      if (!card || !canOpen(card)) return null;
      if (card.collapsed) return chevronIntent(card, { type: "expand", ids: [card.id] });
      // open: its first child, when one is drawn right after it (a function open with its calls not drawn has none)
      return laid.nodes[at + 1] && laid.nodes[at + 1].depth > card.depth ? { type: "select", id: ids[at + 1] } : null;
    case "Enter": // a structure card: Focus. A call card: Explore, activated (03 §7); an unknown callee has nowhere to go
      if (!card) return null;
      return card.role === "node" ? { type: "focus", id: card.id } : card.key === null ? null : { type: "explore", call: card.id, activate: true };
    case " ":
      return card && canOpen(card) ? chevronIntent(card, { type: "toggle", id: card.id }) : null;
    case "Escape":
      return selected ? { type: "select", id: null } : null;
    default:
      return null;
  }
}

/** What Ctrl/Cmd+Enter asks of the page, which is not a change of the session. */
export type Chord = { type: "goToCode"; key: string } | { type: "goToCallSite"; sites: OpenableSite[] };

/**
 * Ctrl/Cmd+Enter (03 §7): on a structure card Go to Code; on a call card Go to Call Site, which is the one site when there is one, and the choice of
 * them when there are several (the host shows the list with a preview). A call whose sites name no file, or an unknown callee, has nowhere to go.
 */
export function canvasChord(key: string, laid: Layout, selected: string | null): Chord | null {
  if (key !== "Enter") return null;
  const card = selected ? laid.nodes.find((p) => p.id === selected) : null;
  if (!card) return null;
  if (card.role === "node") return card.kind === "folder" ? null : { type: "goToCode", key: card.id };
  const sites = listedSites({ call_sites: card.call?.sites ?? [] } as Pick<CallTreeNode, "call_sites"> as CallTreeNode);
  return sites.length === 0 ? null : { type: "goToCallSite", sites };
}
