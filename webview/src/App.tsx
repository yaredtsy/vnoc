import { ReactFlowProvider } from "@xyflow/react";

import { TooltipProvider } from "@/components/ui/tooltip.tsx";
import { useCanvasKeys } from "./app/useCanvasKeys.ts";
import { useFocusedView } from "./app/useFocusedView.ts";
import { pageBody } from "./app/pageBody.ts";
import { useRenderedReport } from "./app/useRenderedReport.ts";
import { paneContext } from "./node-card/nodeContext.ts";
import { Canvas } from "./canvas/Canvas.tsx";
import { CanvasMessage } from "./canvas/CanvasMessage.tsx";
import { TopBar } from "./canvas/top-bar/TopBar.tsx";
import { DocPanel } from "./docs/DocPanel.tsx";
import { on, request } from "./host.ts";
import { useData } from "./state/data.ts";
import "./state/draftFeed.ts"; // the canvas page tells the host its drafts, and takes the host's copy when it starts with none (9.5c)
import { setDocsOpen, useView } from "./state/view.ts";
import { useEffect } from "react";

// The canvas page: the bar, the cards, the doc panel. It reads the stores and draws; nothing here keeps state of its own.

// right-click on the empty canvas: its own menu (Fit View, Collapse All, ...); a card inside it says "node" and wins
const PANE_CONTEXT = paneContext("canvas");

export function App() {
  const { status, message, structure, reloads } = useData();
  const docsOpen = useView((v) => v.docsOpen);
  const { root, top, laid, selected, active, subject } = useFocusedView(structure);
  useCanvasKeys(laid, root, selected);
  // the test suite cannot click the bar's ⓘ: this opens the doc panel the way it does
  useEffect(() => on("viewCommand", ({ name }) => name === "showDocPanel" && setDocsOpen(true)), []);
  useRenderedReport(laid, active);
  const body = pageBody(status, laid !== null);

  return (
    <TooltipProvider>
      <ReactFlowProvider>
        <div className="flex h-screen flex-col">
          <TopBar structure={structure} status={status} message={message} cards={laid ? { drawn: laid.nodes.length, total: laid.total } : null} />
          <div className="flex min-h-0 flex-1" data-vscode-context={PANE_CONTEXT}>
            {/* nothing drawn yet: the message alone. Once cards are drawn they stay, whatever a later refresh says (the bar's ⚠ tells) */}
            {body === "canvas" && laid ? <Canvas key={`${active}:${reloads}:${top?.id ?? ""}`} laid={laid} context={active} at={`${reloads}:${top?.id ?? ""}`} /> : <CanvasMessage status={body === "error" ? "error" : "loading"} message={message} />}
            {docsOpen && (
              <DocPanel
                node={subject.node}
                calledBy={subject.calledBy}
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
