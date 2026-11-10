// The graph view. It draws a layout and reports what the user asked for; it knows nothing about the host or where the tree came from.

import { Background, Controls, MiniMap, type Node, ReactFlow, useNodesState } from "@xyflow/react";
import { useEffect, useMemo } from "react";

import { NodeCard } from "../node-card/NodeCard.tsx";
import { CanvasActionsProvider } from "./CanvasActions.ts";
import type { Layout, Placed } from "./layout.ts";

const nodeTypes = { card: NodeCard };

const toFlow = (laid: Layout, selected: string | null): Node<Placed>[] => laid.nodes.map((p) => ({ id: p.id, type: "card", position: { x: p.x, y: p.y }, data: p, selected: p.id === selected }));

interface CanvasProps {
  laid: Layout;
  /** the card the user clicked last */
  selected: string | null;
  onSelect: (id: string) => void;
  onToggle: (id: string) => void;
  onToggleCode: (id: string) => void;
  onAddDoc: (id: string) => void;
  onShowDocs: (id: string) => void;
  onFocus: (id: string) => void;
}

export function Canvas({ laid, selected, onSelect, onToggle, onToggleCode, onAddDoc, onShowDocs, onFocus }: CanvasProps) {
  // The layout decides where nodes start; dragging moves them in this state. A new layout (expand, collapse, refresh) puts them back in their rows.
  const [nodes, setNodes, onNodesChange] = useNodesState(toFlow(laid, selected)); // starting with the nodes (not []) lets fitView frame them on first paint
  useEffect(() => setNodes(toFlow(laid, selected)), [laid, setNodes]); // eslint-disable-line react-hooks/exhaustive-deps -- a click must not put dragged cards back
  useEffect(() => setNodes((current) => current.map((n) => (n.selected === (n.id === selected) ? n : { ...n, selected: n.id === selected }))), [selected, setNodes]);
  const actions = useMemo(() => ({ toggle: onToggle, toggleCode: onToggleCode, addDoc: onAddDoc, showDocs: onShowDocs }), [onToggle, onToggleCode, onAddDoc, onShowDocs]);
  return (
    <CanvasActionsProvider value={actions}>
      <ReactFlow
        nodes={nodes}
        edges={laid.edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onNodeClick={(_, node) => onSelect(node.id)}
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
