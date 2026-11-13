import { useEffect } from "react";

import type { Layout } from "@/canvas/layout.ts";
import { post } from "@/host.ts";

/** Tells the host what was drawn: the extension's test suite uses it to prove the whole chain ran (bundle, request, core, driver, layout). */
export function useRenderedReport(laid: Layout | null): void {
  useEffect(() => {
    if (laid) post({ type: "rendered", visible: laid.nodes.length, hidden: laid.hidden });
  }, [laid]);
}
