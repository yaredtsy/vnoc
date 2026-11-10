// The graph view. It draws a layout and reports what the user asked for; it knows nothing about the host or where the tree came from.

import { Background, Controls, MiniMap, type Node, ReactFlow, useNodesState } from "@xyflow/react";
import { useEffect, useMemo } from "react";

import { NodeCard } from "../node-card/NodeCard.tsx";
import { CanvasActionsProvider } from "./CanvasActions.ts";
import type { Layout, Placed } from "./layout.ts";

const nodeTypes = { card: NodeCard };

const toFlow = (laid: Layout): Node<Placed>[] => laid.nodes.map((p) => ({ id: p.id, type: "card", position: { x: p.x, y: p.y }, data: p }));

export function Canvas({ laid, onToggle, onToggleCode, onFocus }: { laid: Layout; onToggle: (id: string) => void; onToggleCode: (id: string) => void; onFocus: (id: string) => void }) {
  // The layout decides where nodes start; dragging moves them in this state. A new layout (expand, collapse, refresh) puts them back in their rows.
  const [nodes, setNodes, onNodesChange] = useNodesState(toFlow(laid)); // starting with the nodes (not []) lets fitView frame them on first paint
  useEffect(() => setNodes(toFlow(laid)), [laid, setNodes]);
  const actions = useMemo(() => ({ toggle: onToggle, toggleCode: onToggleCode }), [onToggle, onToggleCode]);
  return (
    <CanvasActionsProvider value={actions}>
      <ReactFlow
        nodes={nodes}
        edges={laid.edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onNodeDoubleClick={(_, node) => onFocus(node.id)}
        zoomOnDoubleClick={false}
        nodesConnectable={false}
        onlyRenderVisibleElements
        fitView
        minZoom={0.02}
        colorMode={document.body.classList.contains("vscode-light") ? "light" : "dark"}
        proOptions={{ hideAttribution: true }}
      >
        <Background />
        <Controls showInteractive={false} />
        <MiniMap pannable zoomable />
      </ReactFlow>
    </CanvasActionsProvider>
  );
}
