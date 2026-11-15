import type { TreeNode } from "@vnoc/shared/messages.ts";
import type { ContextId } from "@vnoc/shared/session.ts";
import { BarButton } from "@/components/BarButton.tsx";
import { Codicon } from "@/components/Codicon.tsx";
import { BackButton } from "../canvas/top-bar/BackButton.tsx";
import { Crumbs } from "../canvas/top-bar/Crumbs.tsx";
import { statsLine } from "../canvas/top-bar/statsLine.ts";
import { useCrumbs } from "../canvas/top-bar/useCrumbs.ts";
import { useElementWidth } from "../canvas/top-bar/useElementWidth.ts";
import type { StructureResult } from "@vnoc/shared/messages.ts";
import { act } from "./actions.ts";

/**
 * The panel's focus bar (06 §3), while its context has a focus: the same path as the canvas's crumbs for the same job, `←` one level up, `✕` clear. The
 * crumbs fold from the left into `…` when it is narrow. Every gesture is `context`'s (the panel's): pressing in the panel has made it the active one already.
 */
export function FocusBar({ structure, focus, context }: { structure: StructureResult; focus: readonly string[]; context: ContextId }) {
  const crumbs = useCrumbs(structure.root as TreeNode, focus);
  const [ref, width] = useElementWidth<HTMLDivElement>();
  return (
    <div ref={ref} className="relative flex h-[30px] flex-none items-center gap-0.5 border-b border-border px-1.5">
      <span className="flex size-[22px] flex-none items-center justify-center text-muted-foreground" title="the sidebar and the canvas are focused on this node">
        <Codicon name="target" className="text-[16px]" />
      </span>
      <Crumbs project={structure.root.name} stats={statsLine(structure)} crumbs={crumbs} width={width} context={context} />
      <BackButton crumbs={crumbs} context={context} />
      <BarButton icon="close" label="Clear focus" onClick={() => act(context, { type: "focusJump", depth: 0 })} />
    </div>
  );
}
