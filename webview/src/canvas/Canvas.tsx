// The graph view. It draws a layout; a click is an intent for the session. It knows nothing about the host or where the tree came from.

import { Background, Controls, MiniMap, type Node, ReactFlow, useNodesState, useReactFlow } from "@xyflow/react";
import { useEffect, useMemo, useState } from "react";

import { NodeCard } from "../node-card/NodeCard.tsx";
import { on } from "../host.ts";
import { dispatch, useSession } from "../state/session.ts";
import { useData } from "../state/data.ts";
import { pulse, setCodeOpen, useView } from "../state/view.ts";
import { holderOf, pathEdges } from "./marks.ts";
import type { Layout, Placed } from "./layout.ts";

const nodeTypes = { card: NodeCard };

const toFlow = (laid: Layout): Node<Placed>[] => laid.nodes.map((p) => ({ id: p.id, type: "card", position: { x: p.x, y: p.y }, data: p }));

/** Brings the requested node to the middle of the view once it is drawn. Inside `<ReactFlow>` so it can reach the view. */
function CenterOn({ laid }: { laid: Layout }) {
  const { fitView } = useReactFlow();
  // the session's request to bring a node into view; done, it says so and the session clears it (a canvas opened later does not repeat it)
  const reveal = useSession((s) => s.reveal);
  const target = useMemo(() => (reveal ? { id: reveal.id, n: reveal.n } : null), [reveal]);
  useEffect(() => {
    if (!target || !laid.nodes.some((p) => p.id === target.id)) return;
    // the cards of a new layout are measured on the next frames; framing them before that frames nothing
    const timer = setTimeout(() => {
      void fitView({ nodes: [{ id: target.id }], duration: 300, maxZoom: 1.2 });
      dispatch({ type: "revealed", n: target.n });
      pulse(target.id); // the card pulses for a second so the eye finds it
    }, 60);
    return () => clearTimeout(timer);
  }, [fitView, target, laid]);
  return null;
}

/** Menu commands that change only what this canvas draws: show or hide a card's code, fit the view. */
function ViewCommands() {
  const { fitView } = useReactFlow();
  useEffect(
    () =>
      on("viewCommand", ({ name, key }) => {
        if (name === "fitView") void fitView({ duration: 300 });
        else if (key) setCodeOpen(key, name === "showCode");
      }),
    [fitView],
  );
  return null;
}

// Module-level, so they are the same function on every render: React Flow re-renders every card when one of its props changes.
const onNodeClick = (_: unknown, node: { id: string }) => dispatch({ type: "select", id: node.id });
const onNodeDoubleClick = (_: unknown, node: { id: string }) => dispatch({ type: "focus", id: node.id });

export function Canvas({ laid }: { laid: Layout }) {
  const root = useData((s) => s.structure?.root ?? null);
  const selected = useSession((s) => s.contexts[s.active].selected);
  const cursor = useSession((s) => s.cursor);

  // The edges from the root of what is drawn down to the selection are thicker: you see where it sits in the tree at a glance.
  const drawnRoot = laid.nodes[0]?.id;
  const edges = useMemo(() => {
    const onPath = root && drawnRoot ? pathEdges(root, drawnRoot, selected) : new Set<string>();
    return onPath.size === 0 ? laid.edges : laid.edges.map((e) => (onPath.has(e.id) ? { ...e, className: "on-path" } : e));
  }, [laid, root, drawnRoot, selected]);

  // A closed card that holds the selection or the cursor shows a dot (the cards read these two ids from the view store).
  const drawnIds = useMemo(() => new Set(laid.nodes.map((p) => p.id)), [laid]);
  const hidesSelected = root ? holderOf(root, drawnIds, selected) : null;
  const hidesCursor = root ? holderOf(root, drawnIds, cursor) : null;
  useEffect(() => {
    useView.setState({ hidesSelected, hidesCursor });
  }, [hidesSelected, hidesCursor]);

  // The layout decides where nodes start; dragging moves them in this state. A new layout (expand, collapse, refresh) puts them back in their rows.
  const [nodes, setNodes, onNodesChange] = useNodesState(toFlow(laid)); // starting with the nodes (not []) lets fitView frame them on first paint
  // Adjusted during the render, not in an effect (React's way to update state from a changed prop: no frame drawn with the old cards).
  // Only a new layout puts the cards back in their rows; a click changes no card (the session draws the ring), so it cannot move a dragged one.
  const [drawn, setDrawn] = useState(laid);
  if (drawn !== laid) {
    setDrawn(laid);
    setNodes(toFlow(laid));
  }
  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      onNodesChange={onNodesChange}
      onNodeClick={onNodeClick}
      onNodeDoubleClick={onNodeDoubleClick}
      elementsSelectable={false} // the session decides what is selected, not React Flow
      zoomOnDoubleClick={false}
      nodesConnectable={false}
      fitView
      minZoom={0.02}
      colorMode={document.body.classList.contains("vscode-light") ? "light" : "dark"}
      proOptions={{ hideAttribution: true }}
    >
      <Background />
      <Controls showInteractive={false} />
      <MiniMap pannable zoomable nodeColor={(node) => (node.id === selected ? "var(--card-selected)" : node.id === cursor ? "var(--card-cursor)" : "var(--muted-foreground)")} />
      <CenterOn laid={laid} />
      <ViewCommands />
    </ReactFlow>
  );
}
