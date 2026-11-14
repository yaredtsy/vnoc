import { useEffect } from "react";

import type { Layout } from "@/canvas/layout.ts";
import { post } from "@/host.ts";

/**
 * Tells the host what was drawn: the extension's test suite uses it to prove the whole chain ran (bundle, request, core, driver, layout). `callCards`: how many
 * call cards are drawn; `chevrons`: how many function and class cards show a chevron for their calls.
 * Also tells it which function and class cards are drawn, in reading order: the host probes the ones whose calls it does not know (`cardsDrawn`).
 */
export function useRenderedReport(laid: Layout | null, selected: string | null): void {
  useEffect(() => {
    if (!laid) return;
    const entries = laid.nodes.filter((n) => n.role === "node" && (n.kind === "function" || n.kind === "class"));
    post({ type: "rendered", visible: laid.nodes.length, hidden: laid.hidden, callCards: laid.nodes.filter((n) => n.role === "call").length, chevrons: entries.filter((n) => n.callsChevron).length });
  }, [laid]);

  // the selected card first, then the others as they are drawn; sent again only when the list changes
  useEffect(() => {
    if (!laid) return;
    const keys = laid.nodes.filter((n) => n.role === "node" && (n.kind === "function" || n.kind === "class") && n.childCount === 0).map((n) => n.id);
    post({ type: "cardsDrawn", keys: selected !== null && keys.includes(selected) ? [selected, ...keys.filter((k) => k !== selected)] : keys });
  }, [laid?.nodes.map((n) => n.id).join("\n"), selected]);
}
