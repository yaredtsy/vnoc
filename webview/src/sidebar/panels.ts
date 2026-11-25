// The panels of the STRUCTURE view (docs/16-ui-redesign/06-sidebar.md §4): one per context, in `session.order` (a child right under its parent). What each
// header says, which one is active, and where `Tab` goes. Pure, so the order and the words are tests; the page draws.

import { type CallId, linesText, parseCallId } from "@vnoc/shared/calls.ts";
import type { TreeNode } from "@vnoc/shared/messages.ts";
import type { ContextId, Session } from "@vnoc/shared/session.ts";
import { findNode } from "@vnoc/shared/tree.ts";

export interface Panel {
  id: ContextId;
  kind: "main" | "explore";
  /** 0 for MAIN, 1 for an EXPLORE of MAIN, 2 for an EXPLORE of that: the splitter's size is kept per depth (06 §4.1) */
  depth: number;
  /** `MAIN`, or `◇ EXPLORE: reserve_stock · from checkout_order` */
  label: string;
  /** `checkout_order calls reserve_stock at line 16`; null for MAIN */
  tooltip: string | null;
  active: boolean;
  /** opened by selecting its call and not activated yet: the header is in italics (09 §4) */
  preview: boolean;
  /** an EXPLORE header has a ✕ */
  closable: boolean;
}

/** The name of the function that makes a call: the step before the last, or the owner of the tree for the first step. A callee the index does not know falls back to the end of its key. */
export function callerName(call: CallId, root: TreeNode | null): string {
  const parsed = parseCallId(call);
  if (!parsed) return "?";
  const key = (parsed.steps.length >= 2 ? parsed.steps[parsed.steps.length - 2] : parsed.owner).replace(/#\d+$/, "");
  return (root && findNode(root, key)?.name) || key.replace(/^\?/, "").split("::").pop() || key;
}

/**
 * The panels in order, and whether their headers are drawn (with two or more contexts: with one there is nothing to tell apart). `linesOf` says the 1-based
 * lines of a call's sites when the call tree is held (the tooltip names them); without it the tooltip names no line.
 */
export function panelsOf(s: Pick<Session, "order" | "contexts" | "active">, root: TreeNode | null, linesOf: (call: CallId) => readonly number[] | null = () => null): { panels: Panel[]; headers: boolean } {
  const depthOf = (id: ContextId): number => {
    let depth = 0;
    for (let c = s.contexts[id]; c && c.parent !== null; c = s.contexts[c.parent]) depth++;
    return depth;
  };
  const panels = s.order.flatMap((id): Panel[] => {
    const c = s.contexts[id];
    if (!c || c.kind === "tour") return []; // a tour has its own section (docs/26-tours, TR18), not a panel of the structure
    if (c.kind === "main" || c.call === null) return [{ id, kind: "main", depth: 0, label: "MAIN", tooltip: null, active: s.active === id, preview: false, closable: false }];
    const caller = callerName(c.call, root);
    const lines = linesOf(c.call);
    return [
      {
        id,
        kind: "explore",
        depth: depthOf(id),
        label: `◇ EXPLORE: ${c.title} · from ${caller}`,
        tooltip: `${caller} calls ${c.title}${lines && lines.length > 0 ? ` at ${linesText(lines)}` : ""}`,
        active: s.active === id,
        preview: c.preview,
        closable: true,
      },
    ];
  });
  return { panels, headers: panels.length >= 2 };
}

/** Where `Tab` (or `Shift+Tab`) goes from a panel's tree: the next panel's, the previous one's; null at the ends, where the browser moves on out of the view. */
export function tabTarget(order: readonly ContextId[], from: ContextId, shift: boolean): ContextId | null {
  const at = order.indexOf(from);
  return at < 0 ? null : (order[shift ? at - 1 : at + 1] ?? null);
}
