import { IconChevronRight, IconCode, IconFileText, IconLink } from "@tabler/icons-react";

import { Button } from "@/components/ui/button.tsx";
import { cn } from "@/lib/utils.ts";
import { useCanvasActions } from "../canvas/CanvasActions.ts";
import type { Placed } from "../canvas/layout.ts";
import { KINDS } from "./kinds.ts";

export function NodeHeader({ node }: { node: Placed }) {
  const { toggle, toggleCode, showDocs } = useCanvasActions();
  const kind = KINDS[node.kind];
  return (
    <div className="node-header">
      {node.childCount > 0 ? (
        // `nodrag nopan`: React Flow must not start a drag or a pan from a click on the button
        <Button
          variant="ghost"
          size="icon-xs"
          className="nodrag nopan size-[18px]"
          aria-expanded={!node.collapsed}
          aria-label={node.collapsed ? `Expand ${node.name}` : `Collapse ${node.name}`}
          onClick={() => toggle(node.id)}
        >
          <IconChevronRight className={cn("size-3.5 transition-transform", !node.collapsed && "rotate-90")} />
        </Button>
      ) : (
        <span className="size-[18px] flex-none" />
      )}
      <span className="icon">{kind.icon}</span>
      <span className="label">{kind.title(node.name)}</span>
      {node.collapsed && <span className="badge">+{node.childCount}</span>}
      {node.link && (
        <span className={`link ${node.link}`} title={node.link === "marker" ? "The ID is in the code: the docs follow it anywhere" : "Found by its stored path and name: the docs are lost if it moves"}>
          <IconLink className="size-3" />
        </span>
      )}
      {node.docs > 0 && (
        <Button variant="secondary" size="xs" className="nodrag nopan h-[18px] rounded-full px-1.5" aria-label={`Show the ${node.docs} docs of ${node.name}`} onClick={() => showDocs(node.id)}>
          <IconFileText className="size-3" /> {node.docs}
        </Button>
      )}
      {node.range && (
        <Button variant={node.codeOpen ? "secondary" : "ghost"} size="icon-xs" className="nodrag nopan size-[18px]" aria-pressed={node.codeOpen} aria-label={`Show the code of ${node.name}`} onClick={() => toggleCode(node.id)}>
          <IconCode className="size-3.5" />
        </Button>
      )}
    </div>
  );
}
