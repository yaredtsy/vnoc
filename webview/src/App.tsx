import { ReactFlowProvider } from "@xyflow/react";

import { TooltipProvider } from "@/components/ui/tooltip.tsx";
import { useCanvasKeys } from "./app/useCanvasKeys.ts";
import { useFocusedView } from "./app/useFocusedView.ts";
import { pageBody } from "./app/pageBody.ts";
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
  const body = pageBody(status, laid !== null);

  return (
    <TooltipProvider>
      <ReactFlowProvider>
        <div className="flex h-screen flex-col">
          <TopBar structure={structure} status={status} message={message} />
          <div className="flex min-h-0 flex-1" data-vscode-context={PANE_CONTEXT}>
            {/* nothing drawn yet: the message alone. Once cards are drawn they stay, whatever a later refresh says (the bar's ⚠ tells) */}
            {body === "canvas" && laid ? <Canvas key={`${reloads}:${top?.id ?? ""}`} laid={laid} /> : <CanvasMessage status={body === "error" ? "error" : "loading"} message={message} />}
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
