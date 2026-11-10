import { IconPlus } from "@tabler/icons-react";

import { Button } from "@/components/ui/button.tsx";
import type { Placed } from "../canvas/layout.ts";
import { useNodeActions } from "./useNodeActions.ts";

export function NodeFooter({ node, contextId }: { node: Placed; contextId: string }) {
  const { addDoc } = useNodeActions(node.id, contextId);
  const where = node.line === null ? node.path : `${node.path}:${node.line}`;
  // the opened folder (the root) is not a node of the project: its id is "/"
  const canAttach = node.id !== "/";
  return (
    <div className="node-footer">
      <span className="where" title={where}>
        {where}
      </span>
      {canAttach && (
        <Button variant="ghost" size="icon-xs" className="nodrag nopan size-4" title="Add a doc to this node" aria-label={`Add a doc to ${node.name}`} onClick={addDoc}>
          <IconPlus className="size-3" />
        </Button>
      )}
    </div>
  );
}
