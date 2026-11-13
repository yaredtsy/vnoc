import { ReactFlowProvider } from "@xyflow/react";

import { TooltipProvider } from "@/components/ui/tooltip.tsx";
import { useCanvasKeys } from "./app/useCanvasKeys.ts";
import { useFocusedView } from "./app/useFocusedView.ts";
import { useRenderedReport } from "./app/useRenderedReport.ts";
import { Canvas } from "./canvas/Canvas.tsx";
import { CanvasMessage } from "./canvas/CanvasMessage.tsx";
import { TopBar } from "./canvas/top-bar/TopBar.tsx";
import { DocPanel } from "./docs/DocPanel.tsx";
import { request } from "./host.ts";
import { useData } from "./state/data.ts";
import { setDocsOpen, useView } from "./state/view.ts";

// The canvas page: the bar, the cards, the doc panel. It reads the stores and draws; nothing here keeps state of its own.

// right-click on the empty canvas: its own menu (Fit View, Collapse All, ...); a card inside it says "node" and wins
const PANE_CONTEXT = JSON.stringify({ webviewSection: "pane", preventDefaultContextMenuItems: true });

export function App() {
  const { status, message, structure, reloads } = useData();
  const docsOpen = useView((v) => v.docsOpen);
  const { root, top, laid, selected, docNode } = useFocusedView(structure);
  useCanvasKeys(laid, root, selected);
  useRenderedReport(laid);

  return (
    <TooltipProvider>
      <ReactFlowProvider>
        <div className="flex h-screen flex-col">
          <TopBar structure={structure} loading={status === "loading"} />
          <div className="flex min-h-0 flex-1" data-vscode-context={PANE_CONTEXT}>
            {status === "error" && <CanvasMessage status="error" message={message} />}
            {status === "loading" && !laid && <CanvasMessage status="loading" message="" />}
            {laid && <Canvas key={`${reloads}:${top?.id ?? ""}`} laid={laid} />}
            {docsOpen && (
              <DocPanel
                node={docNode}
                revision={structure?.revision ?? 0}
                onAddDoc={(key) => void request("addDoc", { key }).catch(() => {})}
                onClose={() => setDocsOpen(false)}
              />
            )}
          </div>
        </div>
      </ReactFlowProvider>
    </TooltipProvider>
  );
}
