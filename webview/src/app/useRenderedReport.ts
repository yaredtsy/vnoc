import { useEffect, useRef } from "react";

import type { Layout } from "@/canvas/layout.ts";
import { post } from "@/host.ts";
import { toursOf, useTourCounts } from "@/state/tourCounts.ts";
import { toursChipTip } from "@/sidebar/tours/onView.ts";

/**
 * Tells the host what was drawn (and, as `tourChips`, which cards show `▶ N`): the extension's test suite uses it to prove the whole chain ran (bundle, request, core, driver, layout). `context`: the one the canvas draws; `callCards`: how many
 * call cards are drawn; `chevrons`: how many function and class cards show a chevron for their calls.
 */
export function useRenderedReport(laid: Layout | null, context: string): void {
  useEffect(() => {
    if (!laid) return;
    const entries = laid.nodes.filter((n) => n.role === "node" && (n.kind === "function" || n.kind === "class"));
    post({ type: "rendered", context, visible: laid.nodes.length, hidden: laid.hidden, callCards: laid.nodes.filter((n) => n.role === "call").length, chevrons: entries.filter((n) => n.callsChevron).length });
  }, [laid, context]);

  const counts = useTourCounts((s) => s.counts);
  const reported = useRef("");
  useEffect(() => {
    if (!laid) return;
    const chips = laid.nodes.filter((n) => n.role === "node").flatMap((n) => (toursOf(counts, n.key) > 0 ? [{ id: n.id, tours: toursOf(counts, n.key), tip: toursChipTip(toursOf(counts, n.key)) }] : []));
    const note = { type: "tourChips" as const, surface: "canvas" as const, chips };
    const text = JSON.stringify(note);
    if (text === reported.current) return;
    reported.current = text;
    post(note);
  }, [laid, counts]);
}
