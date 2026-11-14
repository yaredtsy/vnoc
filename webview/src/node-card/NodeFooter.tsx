import type { ReactNode } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { Button } from "@/components/ui/button.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip.tsx";
import { cn } from "@/lib/utils.ts";
import type { Placed } from "../canvas/layout.ts";
import { useView } from "../state/view.ts";
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

/** The counts of `footerChips`, always in the same place. Draws only: which chips, their text and tips are decided there. */
export function NodeFooter({ node, contextId }: { node: Placed; contextId: string }) {
  const { toggle, showDocs } = useNodeActions(node.id, contextId);
  const hidesSelected = useView((v) => v.hidesSelected === node.id);
  const hidesCursor = useView((v) => v.hidesCursor === node.id);
  const draw = (c: Chip): ReactNode => {
    switch (c.kind) {
      case "times": // calls merged into this card: a fact, not a button
        return <ChipView key={c.kind} tip={c.tip}>{c.text}</ChipView>;
      case "hidden":
        return <ChipView key={c.kind} tip={c.tip} onClick={toggle}>{c.text}</ChipView>;
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
      default: {
        const unhandled: never = c.kind; // a new kind of chip must be drawn: the compiler says so
        return unhandled;
      }
    }
  };
  return <div className="flex h-5 flex-none items-center gap-0.5 pr-1.5 text-muted-foreground" style={{ paddingLeft: NAME_X - CHIP_PAD }}>{footerChips(node, { selected: hidesSelected, cursor: hidesCursor }).map(draw)}</div>;
}
