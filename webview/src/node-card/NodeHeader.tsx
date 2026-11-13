import { Codicon } from "@/components/Codicon.tsx";
import { Button } from "@/components/ui/button.tsx";
import { cn } from "@/lib/utils.ts";
import { type Placed, canOpen } from "../canvas/layout.ts";
import { ActionStrip } from "./ActionStrip.tsx";
import { KINDS } from "./kinds.ts";
import { useNodeActions } from "./useNodeActions.ts";

/** Chevron (or ⌖ on the root of a focused view), kind icon, name, buttons. The opened folder, always open, has no chevron. */
export function NodeHeader({ node, contextId, focusRoot, selected }: { node: Placed; contextId: string; focusRoot: boolean; selected: boolean }) {
  const { toggle } = useNodeActions(node.id, contextId);
  const { icon, title, color } = KINDS[node.kind];
  return (
    <div className="node-header flex h-[30px] flex-none items-center gap-1 pr-1 pl-1">
      {focusRoot ? (
        <span className="flex size-[18px] flex-none items-center justify-center text-[color:var(--card-selected)]" title="the canvas is focused on this node">
          <Codicon name="target" />
        </span>
      ) : node.depth > 0 && canOpen(node) ? (
        <Button
          variant="ghost"
          size="icon-xs"
          className="nodrag nopan size-[18px]"
          aria-expanded={!node.collapsed}
          aria-label={node.collapsed ? `Expand ${node.name}` : `Collapse ${node.name}`}
          onClick={toggle}
        >
          <Codicon name="chevron-right" className={cn("transition-transform", !node.collapsed && "rotate-90")} />
        </Button>
      ) : (
        <span className="size-[18px] flex-none" />
      )}
      <span className="flex-none" style={{ color }}>
        <Codicon name={icon} />
      </span>
      <span className="min-w-0 flex-1 truncate text-[13px] font-semibold" title={title(node.name)}>
        {title(node.name)}
      </span>
      <ActionStrip node={node} contextId={contextId} focusRoot={focusRoot} selected={selected} />
    </div>
  );
}
