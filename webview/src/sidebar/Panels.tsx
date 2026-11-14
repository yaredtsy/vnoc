import { useMemo, useRef } from "react";

import type { StructureResult } from "@vnoc/shared/messages.ts";
import { callFacts } from "@vnoc/shared/calls.ts";
import type { ContextId } from "@vnoc/shared/session.ts";
import { useCalls } from "../state/calls.ts";
import { useSession } from "../state/session.ts";
import { setPanelShares, useView } from "../state/view.ts";
import { ContextPanel } from "./ContextPanel.tsx";
import { panelsOf, tabTarget } from "./panels.ts";
import { DEFAULT_SHARE, dragSplitter, panelHeights, resetSplitter, sharesOf } from "./splitters.ts";
import { Splitter } from "./Splitter.tsx";
import { useElementHeight } from "./useElementHeight.ts";

/**
 * The panels in `session.order`, a child under its parent, with a splitter on the top edge of each one after the first (06 §4.1). One panel fills the view and
 * has no splitter and no header. The sizes are shares of the view's height, kept per depth in the page's state; a drag moves the panels' heights directly (no
 * re-render of the rows while it lasts) and keeps the shares when it ends. `onList` and `onTab` are the lists' keyboard: the page owns where `Tab` goes.
 * Reads the session and draws.
 */
export function Panels({ structure, onList, focusList }: { structure: StructureResult; onList: (id: ContextId, el: HTMLDivElement | null) => void; focusList: (id: ContextId) => void }) {
  const order = useSession((s) => s.order);
  const contexts = useSession((s) => s.contexts);
  const active = useSession((s) => s.active);
  const trees = useCalls((s) => s.trees);
  const stored = useView((v) => v.panelShares);
  const { panels, headers } = useMemo(() => panelsOf({ order, contexts, active }, structure.root, (call) => callFacts(trees, call)?.lines ?? null), [order, contexts, active, structure, trees]);
  const [box, height] = useElementHeight<HTMLDivElement>();
  const shares = panels.slice(1).map((p) => stored[p.depth] ?? DEFAULT_SHARE);
  const layout = panels.length > 1 && height > 0 ? panelHeights(height, shares) : null;
  const sections = useRef<(HTMLElement | null)[]>([]);
  const dragFrom = useRef<number[]>([]);

  const keep = (heights: number[]) => setPanelShares(Object.fromEntries(panels.slice(1).map((p, i) => [String(p.depth), sharesOf(height, heights)[i]])));
  const show = (heights: number[]) => heights.forEach((h, i) => sections.current[i] && (sections.current[i]!.style.height = `${h}px`));

  return (
    <div ref={box} className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      {panels.map((panel, i) => (
        <ContextPanel
            key={panel.id}
            sectionRef={(el) => void (sections.current[i] = el)}
            panel={panel}
            headers={headers}
            structure={structure}
            height={layout ? layout.heights[i] : null}
            splitter={
              i > 0 && layout ? (
                <Splitter
                  onStart={() => (dragFrom.current = layout.heights)}
                  onMove={(dy) => show(dragSplitter(dragFrom.current, i, dy))}
                  onEnd={(dy) => keep(dragSplitter(dragFrom.current, i, dy))}
                  onReset={() => keep(resetSplitter(height, layout.heights, i))}
                />
              ) : null
            }
            onList={(el) => onList(panel.id, el)}
            onTab={(shift) => {
              const to = tabTarget(order, panel.id, shift);
              if (to === null || !panels.some((p) => p.id === to)) return false;
              focusList(to);
              return true;
            }}
          />
      ))}
    </div>
  );
}
