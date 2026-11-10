import type { Placed } from "../canvas/layout.ts";
import { KINDS } from "./kinds.ts";

/** For now "Function top". Later: the first line of the node's first doc (plan 09-02). */
export function NodeBody({ node }: { node: Placed }) {
  return <div className="node-body">{`${KINDS[node.kind].label} ${node.name}`}</div>;
}
