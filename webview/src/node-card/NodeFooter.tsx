import { IconPlus } from "@tabler/icons-react";

import { Button } from "@/components/ui/button.tsx";
import { useCanvasActions } from "../canvas/CanvasActions.ts";
import type { Placed } from "../canvas/layout.ts";

export function NodeFooter({ node }: { node: Placed }) {
  const { addDoc } = useCanvasActions();
  const where = node.line === null ? node.path : `${node.path}:${node.line}`;
  // 0.1 attaches docs to functions and classes; files and folders follow
  const canAttach = node.kind === "function" || node.kind === "class";
  return (
    <div className="node-footer">
      <span className="where" title={where}>
        {where}
      </span>
      {canAttach && (
        <Button variant="ghost" size="icon-xs" className="nodrag nopan size-4" title="Add a doc to this node" aria-label={`Add a doc to ${node.name}`} onClick={() => addDoc(node.id)}>
          <IconPlus className="size-3" />
        </Button>
      )}
    </div>
  );
}
