import type { TreeNode } from "@vnoc/shared/messages.ts";
import { MAIN } from "@vnoc/shared/session.ts";
import { BarButton } from "@/components/BarButton.tsx";
import { Codicon } from "@/components/Codicon.tsx";
import { BackButton } from "../canvas/top-bar/BackButton.tsx";
import { Crumbs } from "../canvas/top-bar/Crumbs.tsx";
import { statsLine } from "../canvas/top-bar/statsLine.ts";
import { useCrumbs } from "../canvas/top-bar/useCrumbs.ts";
import { useElementWidth } from "../canvas/top-bar/useElementWidth.ts";
import type { StructureResult } from "@vnoc/shared/messages.ts";
import { act, activateMain } from "./actions.ts";

/**
 * The panel's focus bar (06 §3), while its context has a focus: the same path as the canvas's crumbs for the same job, `←` one level up, `✕` clear. The
 * crumbs fold from the left into `…` when it is narrow. Every gesture is MAIN's: pressing in the bar makes MAIN the active context first.
 */
export function FocusBar({ structure, focus }: { structure: StructureResult; focus: readonly string[] }) {
  const crumbs = useCrumbs(structure.root as TreeNode, focus);
  const [ref, width] = useElementWidth<HTMLDivElement>();
  return (
    <div ref={ref} onPointerDownCapture={activateMain} className="relative flex h-[30px] flex-none items-center gap-0.5 border-b border-border px-1.5">
      <span className="flex size-[22px] flex-none items-center justify-center text-muted-foreground" title="the sidebar and the canvas are focused on this node">
        <Codicon name="target" className="text-[16px]" />
      </span>
      <Crumbs project={structure.root.name} stats={statsLine(structure)} crumbs={crumbs} width={width} context={MAIN} />
      <BackButton crumbs={crumbs} context={MAIN} />
      <BarButton icon="close" label="Clear focus" onClick={() => act({ type: "focusJump", depth: 0 })} />
    </div>
  );
}
