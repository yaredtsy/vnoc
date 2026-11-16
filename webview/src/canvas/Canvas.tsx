// The graph view. It draws a layout; a click is an intent for the session. It knows nothing about the host or where the tree came from.

import { Background, Controls, MiniMap, type Node, ReactFlow, useNodesState, useReactFlow, useStore } from "@xyflow/react";
import { useEffect, useMemo, useRef, useState } from "react";

import { motionMs, prefersReducedMotion } from "../lib/motion.ts";
import { NodeCard } from "../node-card/NodeCard.tsx";
import { on, post } from "../host.ts";
import { dispatch, useSession } from "../state/session.ts";
import { useData } from "../state/data.ts";
import { checkKeys } from "../node-card/keyCheck.ts";
import { typeInto } from "../state/drafts.ts";
import { beginEdit, cardRef, saveDraft } from "../state/edit.ts";
import { rememberViewport, savedViewport } from "../state/viewports.ts";
import { pulse, setCodeOpen, useView } from "../state/view.ts";
import { doubleClickIntent } from "./cardClicks.ts";
import { FIT_VIEW_EVENT } from "./keys.ts";
import { SETTLE_MS, cardsReport, onScreenIds } from "./onScreen.ts";
import { holderOf, pathEdges } from "./marks.ts";
import { CARD_W, type Layout, type Placed, cardHeight } from "./layout.ts";

const nodeTypes = { card: NodeCard };

// the size is known from the layout, so a card off screen can be left out of the page before it was ever measured (`onlyRenderVisibleElements`)
const toFlow = (laid: Layout): Node<Placed>[] => laid.nodes.map((p) => ({ id: p.id, type: "card", position: { x: p.x, y: p.y }, data: p, width: CARD_W, height: cardHeight(p) }));

/** Brings the requested node to the middle of the view once it is drawn. Inside `<ReactFlow>` so it can reach the view. */
function CenterOn({ laid, context }: { laid: Layout; context: string }) {
  const { fitView } = useReactFlow();
  // the session's request to bring a node into view; done, it says so and the session clears it (a canvas opened later does not repeat it)
  const reveal = useSession((s) => s.reveal);
  // a canvas that shows another context leaves the request for that context's own canvas
  const target = useMemo(() => (reveal && reveal.context === context ? { id: reveal.id, n: reveal.n } : null), [reveal, context]);
  useEffect(() => {
    if (!target || !laid.nodes.some((p) => p.id === target.id)) return;
    // the cards of a new layout are measured on the next frames; framing them before that frames nothing
    const timer = setTimeout(() => {
      void fitView({ nodes: [{ id: target.id }], duration: motionMs(300, prefersReducedMotion()), maxZoom: 1.2 });
      dispatch({ type: "revealed", n: target.n });
      pulse(target.id); // the card pulses for a second so the eye finds it
    }, 60);
    return () => clearTimeout(timer);
  }, [fitView, target, laid]);
  return null;
}

/**
 * Tells the host which function and class cards are on screen (the viewport and a screen of margin), the selected one first, once a pan or zoom has
 * stayed still for SETTLE_MS. The host probes those and no others (03 §6.1). Inside `<ReactFlow>` to read its viewport.
 */
function OnScreenReport({ laid, selected }: { laid: Layout; selected: string | null }) {
  const x = useStore((s) => s.transform[0]);
  const y = useStore((s) => s.transform[1]);
  const zoom = useStore((s) => s.transform[2]);
  const width = useStore((s) => s.width);
  const height = useStore((s) => s.height);
  const last = useRef("");
  useEffect(() => {
    const timer = setTimeout(() => {
      const visible = new Set(onScreenIds(laid.nodes.map((p) => ({ id: p.id, x: p.x, y: p.y, h: cardHeight(p) })), { x, y, zoom }, { width, height }));
      const report = cardsReport(laid.nodes, visible, selected);
      const same = `${report.keys.join("\n")}\n\n${report.onScreen.join("\n")}`;
      if (same === last.current) return;
      last.current = same;
      post({ type: "cardsDrawn", ...report });
    }, SETTLE_MS);
    return () => clearTimeout(timer);
  }, [laid, selected, x, y, zoom, width, height]);
  return null;
}

/** Menu commands (and the F key) that change only what this canvas draws: show or hide a card's code, fit the view, start an edit (the card shows its code first). */
function ViewCommands({ laid }: { laid: Layout }) {
  const { fitView } = useReactFlow();
  const cards = useRef(laid.nodes); // the latest layout, without listening again each time it changes
  cards.current = laid.nodes;
  useEffect(() => {
    const fit = () => void fitView({ duration: motionMs(300, prefersReducedMotion()) });
    const stop = on("viewCommand", ({ name, key, text }) => {
      if (name === "fitView") fit();
      else if (name === "checkKeys") post({ type: "keysChecked", ...checkKeys() });
      else if (key && (name === "edit" || name === "setDraft" || name === "save")) {
        const card = cards.current.find((p) => p.id === key);
        if (!card) return;
        if (name === "edit") void beginEdit(cardRef(card));
        // the suite cannot type or press keys in a webview: these two are the draft's text and Save, as a person's typing and ⌘S would make them
        else if (name === "setDraft" && card.key !== null && text !== undefined) typeInto(card.key, text);
        else if (name === "save" && card.key !== null) void saveDraft(card.key);
      } else if (key) setCodeOpen(key, name === "showCode");
    });
    window.addEventListener(FIT_VIEW_EVENT, fit); // the F key (canvas/keys.ts)
    return () => {
      stop();
      window.removeEventListener(FIT_VIEW_EVENT, fit);
    };
  }, [fitView]);
  return null;
}

// Module-level, so they are the same function on every render: React Flow re-renders every card when one of its props changes.
const onNodeClick = (_: unknown, node: { id: string }) => dispatch({ type: "select", id: node.id });
const onNodeDoubleClick = (_: unknown, node: { data: unknown }) => dispatch(doubleClickIntent(node.data as Placed));

/**
 * `context`: the one it draws; `at`: which drawing this is (the same root of the same load), so its pan and zoom are remembered for as long as the drawing is
 * the same and the context is open (canvas/viewports.ts). A context shown for the first time is fitted.
 */
export function Canvas({ laid, context, at }: { laid: Layout; context: string; at: string }) {
  const saved = useMemo(() => savedViewport(context, at), [context, at]);
  const root = useData((s) => s.structure?.root ?? null);
  const selected = useSession((s) => s.contexts[s.active].selected);
  const cursor = useSession((s) => s.cursor);
  const minimap = useView((v) => v.minimap);

  // The edges from the root of what is drawn down to the selection are thicker: you see where it sits in the tree at a glance.
  const drawnRoot = laid.nodes[0]?.id;
  const edges = useMemo(() => {
    const onPath = root && drawnRoot ? pathEdges(root, drawnRoot, selected) : new Set<string>();
    // a call edge is dashed (03 §6); the way down to the selection is thicker, whichever kind the edge is
    return laid.edges.map((e) => ({ id: e.id, source: e.source, target: e.target, className: [e.kind === "call" && "call", onPath.has(e.id) && "on-path"].filter(Boolean).join(" ") || undefined }));
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
      onlyRenderVisibleElements // a card or edge off screen is not in the page (03 §6.1)
      elementsSelectable={false} // the session decides what is selected, not React Flow
      zoomOnDoubleClick={false}
      nodesConnectable={false}
      fitView={saved === null}
      fitViewOptions={{ maxZoom: 1 }} // a context of one card (a new EXPLORE) would otherwise open at React Flow's limit, 200 %
      defaultViewport={saved ?? undefined}
      onMoveEnd={(_, viewport) => rememberViewport(context, at, viewport)}
      className="min-w-0 flex-1"
      minZoom={0.02}
      proOptions={{ hideAttribution: true }}
    >
      <Background />
      <Controls showInteractive={false} />
      {minimap && <MiniMap pannable zoomable nodeColor={(node) => (node.id === selected ? "var(--card-selected)" : node.id === cursor ? "var(--card-cursor)" : "var(--muted-foreground)")} />}
      <CenterOn laid={laid} context={context} />
      <OnScreenReport laid={laid} selected={selected} />
      <ViewCommands laid={laid} />
    </ReactFlow>
  );
}
