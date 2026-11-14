import type { ReactNode } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { Button } from "@/components/ui/button.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip.tsx";
import { cn } from "@/lib/utils.ts";
import { FOOTER_H, LINE_H, type Placed } from "../canvas/layout.ts";
import { request } from "../host.ts";
import { useView } from "../state/view.ts";
import { chevronAction } from "./chevronAction.ts";
import { type Chip, footerChips } from "./footerChips.ts";
import { CHIP_PAD, NAME_X } from "./rows.ts";
import { useNodeActions } from "./useNodeActions.ts";

/** One count with its tooltip. A button when `onClick` is given; otherwise it only says a fact. */
function ChipView({ tip, onClick, className, children }: { tip: string; onClick?: () => void; className?: string; children: ReactNode }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          onClick ? (
            <Button variant="ghost" size="xs" className="nodrag nopan h-[18px] gap-1 px-1.5 text-[11px] text-muted-foreground" onClick={onClick} />
          ) : (
            <span className={cn("flex h-[18px] items-center gap-1 px-1.5 text-[11px]", className)} />
          )
        }
      >
        {children}
      </TooltipTrigger>
      <TooltipContent>{tip}</TooltipContent>
    </Tooltip>
  );
}

/** The calls' state, on the footer's right: a fact; a spinner while fetching; a failure with Retry. */
function CallsState({ chip, onRetry }: { chip: Chip; onRetry: () => void }) {
  if (chip.calls === "loading") {
    return (
      <ChipView tip={chip.tip}>
        <Codicon name="loading" className="codicon-modifier-spin text-[12px]" />
      </ChipView>
    );
  }
  if (chip.calls === "failed") {
    return (
      <span className="flex h-[18px] items-center gap-1 px-1.5 text-[11px] text-destructive">
        <span title={chip.tip}>{chip.text}</span>
        <span aria-hidden>·</span>
        <Button variant="ghost" size="xs" className="nodrag nopan h-[18px] px-1 text-[11px] text-[color:var(--link)]" onClick={onRetry}>
          Retry
        </Button>
      </span>
    );
  }
  return <ChipView tip={chip.tip}>{chip.text}</ChipView>;
}

/** The counts of `footerChips`, always in the same place. Draws only: which chips, their text and tips are decided there. */
export function NodeFooter({ node, contextId }: { node: Placed; contextId: string }) {
  const actions = useNodeActions(node.id, contextId);
  const { showDocs } = actions;
  const hidesSelected = useView((v) => v.hidesSelected === node.id);
  const hidesCursor = useView((v) => v.hidesCursor === node.id);
  const draw = (c: Chip): ReactNode => {
    switch (c.kind) {
      case "times": // calls merged into this card: a fact, not a button
        return <ChipView key={c.kind} tip={c.tip}>{c.text}</ChipView>;
      case "hidden":
        return <ChipView key={c.kind} tip={c.tip} onClick={actions[chevronAction(node)]}>{c.text}</ChipView>;
      case "docs":
        return (
          <ChipView key={c.kind} tip={c.tip} onClick={showDocs}>
            <Codicon name="file" className="text-[12px]" /> {c.text}
          </ChipView>
        );
      case "link":
        return (
          <ChipView key={c.kind} tip={c.tip} className={cn(c.faded && "opacity-40")}>
            <Codicon name="link" className="text-[12px]" />
          </ChipView>
        );
      case "selected":
        return <ChipView key={c.kind} tip={c.tip} className="text-[color:var(--card-selected)]">{c.text}</ChipView>;
      case "cursor":
        return <ChipView key={c.kind} tip={c.tip} className="text-[color:var(--card-cursor)]">{c.text}</ChipView>;
      case "calls":
        return <CallsState key={c.kind} chip={c} onRetry={() => node.key !== null && void request("retryCalls", { key: node.key }).catch(() => {})} />;
      default: {
        const unhandled: never = c.kind; // a new kind of chip must be drawn: the compiler says so
        return unhandled;
      }
    }
  };
  const chips = footerChips(node, { selected: hidesSelected, cursor: hidesCursor });
  return (
    <>
      {/* the 1 px line above the footer runs the card's full inner width */}
      <div className="flex-none" style={{ height: LINE_H, background: "color-mix(in srgb, var(--card-border) 60%, transparent)" }} />
      {/* 23 px; the card's own 1 px border is inside its 80, so with nothing taller above, the footer gives up the last 2 */}
      <div className="flex min-h-[21px] flex-[0_1_auto] items-center gap-0.5 pr-1.5 text-muted-foreground" style={{ height: FOOTER_H, paddingLeft: NAME_X - CHIP_PAD }}>
        {chips.filter((c) => c.kind !== "calls").map(draw)}
        <span className="min-w-0 flex-1" />
        {chips.filter((c) => c.kind === "calls").map(draw)}
      </div>
    </>
  );
}
