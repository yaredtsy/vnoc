import { useEffect } from "react";

import type { Layout } from "@/canvas/layout.ts";
import { post } from "@/host.ts";

/**
 * Tells the host what was drawn: the extension's test suite uses it to prove the whole chain ran (bundle, request, core, driver, layout). `context`: the one the canvas draws; `callCards`: how many
 * call cards are drawn; `chevrons`: how many function and class cards show a chevron for their calls.
 */
export function useRenderedReport(laid: Layout | null, context: string): void {
  useEffect(() => {
    if (!laid) return;
    const entries = laid.nodes.filter((n) => n.role === "node" && (n.kind === "function" || n.kind === "class"));
    post({ type: "rendered", context, visible: laid.nodes.length, hidden: laid.hidden, callCards: laid.nodes.filter((n) => n.role === "call").length, chevrons: entries.filter((n) => n.callsChevron).length });
  }, [laid, context]);
}
