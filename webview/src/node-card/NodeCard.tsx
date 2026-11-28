// The node card: header, meta line, code, calls, footer (docs/16-ui-redesign/04-node-cards.md). Each part is its own component, so this file is a layout.

import { Handle, type Node, type NodeProps, Position } from "@xyflow/react";
import { memo } from "react";

import { cn } from "@/lib/utils.ts";
import { type Placed, cardHeight, canOpen } from "../canvas/layout.ts";
import { dispatch } from "../state/session.ts";
import { toursOf, useTourCounts } from "../state/tourCounts.ts";
import { cardLabel } from "./labels.ts";
import { KINDS } from "./kinds.ts";
import { nodeContext } from "./nodeContext.ts";
import { CallMeta } from "./CallMeta.tsx";
import { NodeCode } from "./NodeCode.tsx";
import { NodeFooter } from "./NodeFooter.tsx";
import { NodeHeader } from "./NodeHeader.tsx";
import { NodeMeta } from "./NodeMeta.tsx";
import { SiteList } from "./SiteList.tsx";
import { useCardState } from "./useCardState.ts";
import "./node-card.css";

// memo: React Flow passes the same props to a card that did not change, and then the card is not drawn at all.
export const NodeCard = memo(function NodeCard({ data }: NodeProps<Node<Placed>>) {
  const { contextId, selected, focusRoot, cursor, revealed, highlighted, searchedOut } = useCardState(data.id, data.depth);
  const tours = useTourCounts((s) => (data.role === "node" ? toursOf(s.counts, data.key) : 0));
  return (
    <div
      className={cn("node-card", data.role === "call" && "call", data.call?.look.unknown && "opacity-60", data.syntaxError && "broken", selected && "selected", highlighted && "highlighted", cursor && "cursor", revealed && "revealed", searchedOut && "searched-out")}
      // the stripe says what the node IS, never a state
      style={{ width: data.width, height: cardHeight(data), borderLeftColor: KINDS[data.kind].color }}
      aria-selected={selected}
      aria-label={cardLabel({ role: data.role, kind: data.kind, name: data.name, docs: data.docs, lines: data.call?.lines ?? [], unknown: data.call?.look.unknown === true, canOpen: canOpen(data), collapsed: data.collapsed, syntaxError: data.syntaxError }, { selected, cursor })}
      // a right-click selects the card first, so the ring shows what the menu is about (as in the Explorer); selecting never opens anything
      onContextMenu={() => dispatch({ type: "select", id: data.id }, contextId)}
      // VS Code's own right-click menu for a webview: the entries are in package.json (`webview/context`), shown when these keys match
      data-vscode-context={nodeContext({ node: data, contextId, isFocusRoot: focusRoot, tours })}
    >
      {/* the edges attach here; nobody connects nodes by hand, so the dots are invisible (node-card.css) */}
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <NodeHeader node={data} contextId={contextId} focusRoot={focusRoot} selected={selected} />
      {data.role === "call" ? <CallMeta node={data} /> : <NodeMeta node={data} />}
      {data.siteRows > 0 && <SiteList node={data} />}
      {data.codeOpen && <NodeCode node={data} />}
      <NodeFooter node={data} contextId={contextId} />
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </div>
  );
});
