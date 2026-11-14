import { Codicon } from "@/components/Codicon.tsx";
import { Button } from "@/components/ui/button.tsx";
import { cn } from "@/lib/utils.ts";
import { HEADER_H, type Placed, canOpen } from "../canvas/layout.ts";
import { ActionStrip } from "./ActionStrip.tsx";
import { KINDS, kindIcon } from "./kinds.ts";
import { CHEVRON, HEADER_PAD, KIND_ICON } from "./rows.ts";
import { useNodeActions } from "./useNodeActions.ts";

/** Chevron (or ⌖ on the root of a focused view), kind icon, name, buttons. The opened folder, always open, has no chevron. */
export function NodeHeader({ node, contextId, focusRoot, selected }: { node: Placed; contextId: string; focusRoot: boolean; selected: boolean }) {
  const { toggle } = useNodeActions(node.id, contextId, node.key);
  const { title, color } = KINDS[node.kind];
  const icon = kindIcon(node.kind, node.inClass); // a call card shows its callee's kind, never the call icon
  return (
    <div className="node-header flex flex-none items-center gap-1 pr-1" style={{ height: HEADER_H, paddingLeft: HEADER_PAD }}>
      {focusRoot ? (
        <span className="flex flex-none items-center justify-center text-[color:var(--card-selected)]" style={{ width: CHEVRON, height: CHEVRON }} title="the canvas is focused on this node">
          <Codicon name="target" />
        </span>
      ) : node.depth > 0 && canOpen(node) ? (
        <Button
          variant="ghost"
          size="icon-xs"
          className="nodrag nopan" style={{ width: CHEVRON, height: CHEVRON }}
          aria-expanded={!node.collapsed}
          aria-label={node.collapsed ? `Expand ${node.name}` : `Collapse ${node.name}`}
          onClick={toggle}
        >
          <Codicon name="chevron-right" className={cn("transition-transform", !node.collapsed && "rotate-90")} />
        </Button>
      ) : (
        <span className="flex-none" style={{ width: CHEVRON }} />
      )}
      <span className="flex flex-none items-center justify-center" style={{ color, width: KIND_ICON }}>
        <Codicon name={icon} />
      </span>
      <span className="min-w-0 flex-1 truncate text-[13px] font-semibold" title={node.role === "call" ? node.name : title(node.name)}>
        {node.role === "call" ? node.name : title(node.name)}
      </span>
      <ActionStrip node={node} contextId={contextId} focusRoot={focusRoot} selected={selected} />
    </div>
  );
}
