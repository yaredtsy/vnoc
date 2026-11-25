import { Handle, type Node, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { cn } from "@/lib/utils.ts";
import type { Kind } from "@vnoc/shared/messages.ts";
import { findNode } from "@vnoc/shared/tree.ts";
import { CARD_H, CARD_W, FOOTER_H, HEADER_H, META_H } from "../canvas/layout.ts";
import { KINDS } from "../node-card/kinds.ts";
import { tourStopContext } from "../node-card/nodeContext.ts";
import { useData } from "../state/data.ts";
import { useView } from "../state/view.ts";
import { goToStop } from "./goToCode.ts";
import type { TourCardData } from "./map.ts";
import { useTour } from "./store.ts";
import "./tour-map.css";

const GLYPH = { seen: "✓", here: "●", ahead: "", planned: "◌" } as const;
const fileName = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** `3 steps`, `1 doc`: what the footer's left says. */
const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/**
 * A stop of the tour as a card (design 04 §3): the redesign's card, with the stop's badge where the chevron was (`✓1` seen, `●3` here, ` 4` ahead, `◌6` not written),
 * `↗` as its only button, the meta line unchanged (a stop reached by a call is a call card: `↳ line 16` and the callee's file), and a footer of `3 steps`, the docs, and
 * one word on the right. A click is a jump to the stop (the canvas sends it); the ring is on the current stop. Everything is drawn from the `TourCardData`.
 */
export const TourCard = memo(function TourCard({ data }: NodeProps<Node<TourCardData>>) {
  const kind = KINDS[(data.kind && data.kind in KINDS ? data.kind : "function") as Kind];
  // the pointer is over this stop's crumb: the card is pointed at (as the structure's cards are)
  const highlighted = useView((v) => v.hover === data.id);
  const docs = useData((s) => (s.structure && data.key ? (findNode(s.structure.root, data.key)?.docs ?? 0) : 0));
  const where = data.callLine !== null ? null : data.kind === "folder" ? `${data.path}/` : data.line !== null ? `${fileName(data.path)}:${data.line}` : data.path;
  const goToCode = (e: React.MouseEvent) => {
    e.stopPropagation(); // the card's own click is a jump
    const view = useTour.getState().view;
    const stop = view && "stops" in view ? view.stops.find((s) => s.id === data.id) : undefined;
    if (stop) goToStop(useData.getState().structure?.root ?? null, stop);
  };
  return (
    <div
      className={cn("node-card tour-card", data.callLine !== null && "call", data.here && "selected", highlighted && !data.here && "highlighted", data.planned && "tour-planned", data.writing && "tour-writing", data.mark === "seen" && "tour-seen")}
      style={{ width: CARD_W, height: CARD_H, borderLeftColor: kind.color }}
      data-stop={data.id}
      data-mark={data.mark}
      data-writing={data.writing}
      data-vscode-context={tourStopContext({ tour: useTour.getState().id ?? "", stop: data.id, nodeKey: data.key, surface: "canvas" })}
      aria-current={data.here ? "step" : undefined}
      aria-label={`Stop ${data.number}: ${data.name}${data.here ? ", you are here" : ""}`}
    >
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <div className="node-header flex flex-none items-center gap-1 pr-1 pl-1.5" style={{ height: HEADER_H }}>
        <span className={cn("tour-badge", `tour-badge-${data.mark}`)} title={data.planned ? "Not written yet" : data.here ? "You are here" : data.mark === "seen" ? "Seen" : "Ahead"}>
          {GLYPH[data.mark]}
          {data.number}
        </span>
        <span className="flex w-4 flex-none items-center justify-center" style={{ color: kind.color }}>
          <Codicon name={kind.icon} />
        </span>
        <span className={cn("min-w-0 flex-1 truncate text-[13px] font-semibold", data.struck && "line-through")} title={kind.title(data.name)}>
          {kind.title(data.name)}
        </span>
        <button type="button" className="nodrag nopan flex size-5 flex-none items-center justify-center rounded-sm hover:bg-toolbar-hover" title="Go to Code" aria-label={`Go to Code: ${data.name}`} onClick={goToCode}>
          <Codicon name="link-external" className="text-[13px]" />
        </button>
      </div>
      <div className="flex flex-none items-center gap-2 pr-3 pl-[30px] font-mono text-[11px] text-muted-foreground" style={{ height: META_H }}>
        {data.callLine !== null ? (
          <>
            <span className="flex items-center gap-1 whitespace-nowrap">
              <Codicon name="call-outgoing" className="text-[11px]" />
              line {data.callLine}
            </span>
            <span className="ml-auto truncate">{fileName(data.path)}</span>
          </>
        ) : (
          <span className="truncate">{where}</span>
        )}
      </div>
      <div className="h-px flex-none bg-[color:var(--card-border)]" />
      <div className="flex flex-none items-center gap-2 pr-2 pl-[30px] text-[11px] text-muted-foreground" style={{ height: FOOTER_H }}>
        {data.steps !== null && <span>{plural(data.steps, "step", "steps")}</span>}
        {docs > 0 && <span>{plural(docs, "doc", "docs")}</span>}
        <span className="ml-auto">{data.explainedAt !== null ? `⇢ explained at ${data.explainedAt}` : data.description}</span>
      </div>
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </div>
  );
});
