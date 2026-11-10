import type { Placed } from "../canvas/layout.ts";

export function NodeFooter({ node }: { node: Placed }) {
  const where = node.line === null ? node.path : `${node.path}:${node.line}`;
  return (
    <div className="node-footer" title={where}>
      {where}
    </div>
  );
}
