// The node card: header, body, footer (plan_vs/02-technology/03-frontend-webview.md, "The node card"). Each part is its own component,
// so the later controls (link icon, doc count, code toggle) go into the part they belong to and this file stays a layout.

import { Handle, type Node, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";

import type { Placed } from "../canvas/layout.ts";
import { dispatch, useSession } from "../state/session.ts";
import { useView } from "../state/view.ts";
import { nodeContext } from "./nodeContext.ts";
import { NodeCode } from "./NodeCode.tsx";
import { NodeBody } from "./NodeBody.tsx";
import { NodeFooter } from "./NodeFooter.tsx";
import { NodeHeader } from "./NodeHeader.tsx";
import "./node-card.css";

// The card draws its own state from the session: each selector returns a boolean, so a click draws again only the two cards whose state changed
// (memo: React Flow passes the same props to a card that did not change, and then the card is not drawn at all).
export const NodeCard = memo(function NodeCard({ data }: NodeProps<Node<Placed>>) {
  const contextId = useSession((s) => s.active);
  const selected = useSession((s) => s.contexts[s.active].selected === data.id);
  // the card at the top of a focused view (the focused node is always the first card drawn)
  const isFocusRoot = useSession((s) => s.contexts[s.active].focus.length > 0) && data.depth === 0;
  // pointing without selecting (the search list, a crumb under the pointer), the text cursor, and a card just brought into view
  // both hooks are always called: `a || useX()` would skip the second one when the first is true, and React needs the same hooks on every render
  const pointedAt = useSession((s) => s.highlight === data.id);
  const hovered = useView((v) => v.hover === data.id);
  const highlighted = pointedAt || hovered;
  const cursor = useSession((s) => s.cursor === data.id);
  const revealed = useView((v) => v.revealed === data.id);
  return (
    <div
      className={`node-card ${data.kind}${data.syntaxError ? " broken" : ""}${data.codeOpen ? " code-open" : ""}${selected ? " selected" : ""}${highlighted ? " highlighted" : ""}${cursor ? " cursor" : ""}${revealed ? " revealed" : ""}`}
      aria-selected={selected}
      aria-label={`${data.kind} ${data.name}${selected ? ", selected" : ""}${cursor ? ", cursor here" : ""}${data.docs > 0 ? `, ${data.docs} docs` : ""}`}
      // a right-click selects the card first, so the ring shows what the menu is about (as in the Explorer); selecting never opens anything
      onContextMenu={() => dispatch({ type: "select", id: data.id }, contextId)}
      // VS Code's own right-click menu for a webview: the entries are in package.json (`webview/context`), shown when these keys match
      data-vscode-context={nodeContext({ node: data, contextId, isFocusRoot })}
    >
      {/* the edges attach here; nobody connects nodes by hand, so the dots are invisible (node-card.css) */}
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <NodeHeader node={data} contextId={contextId} cursor={cursor} focusRoot={isFocusRoot} />
      {data.codeOpen ? <NodeCode node={data} /> : <NodeBody node={data} />}
      <NodeFooter node={data} contextId={contextId} />
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </div>
  );
});
