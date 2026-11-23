// The graph view. It draws a layout; a click is an intent for the session. It knows nothing about the host or where the tree came from.

import { Background, Controls, MiniMap, type Node, ReactFlow, useNodesState, useReactFlow, useStore, useStoreApi } from "@xyflow/react";
import { useEffect, useMemo, useRef, useState } from "react";

import { motionMs, prefersReducedMotion } from "../lib/motion.ts";
import { NodeCard } from "../node-card/NodeCard.tsx";
import { on, post } from "../host.ts";
import { dispatch, useSession } from "../state/session.ts";
import { useData } from "../state/data.ts";
import { checkKeys } from "../node-card/keyCheck.ts";
import { cardRef, saveDraft, saveFile, typeIn } from "../state/edit.ts";
import { rememberViewport, savedViewport } from "../state/viewports.ts";
import { pulse, requestCodeFocus, setCodeOpen, useView } from "../state/view.ts";
import { doubleClickIntent } from "./cardClicks.ts";
import { FIT_VIEW_EVENT } from "./keys.ts";
import { SETTLE_MS, cardsReport, onScreenIds } from "./onScreen.ts";
import { followsReveal, needsFraming } from "./revealFollow.ts";
import { holderOf, pathEdges } from "./marks.ts";
import { useSearchDim } from "./useSearchDim.ts";
import { CARD_W, type Layout, type Placed, cardHeight } from "./layout.ts";

const nodeTypes = { card: NodeCard };

// the size is known from the layout, so a card off screen can be left out of the page before it was ever measured (`onlyRenderVisibleElements`)
const toFlow = (laid: Layout): Node<Placed>[] => laid.nodes.map((p) => ({ id: p.id, type: "card", position: { x: p.x, y: p.y }, data: p, width: p.width, height: cardHeight(p) }));

/**
 * Brings the requested node into view once it is drawn. Inside `<ReactFlow>` so it can reach the view. The camera moves only when it has to (`revealFollow.ts`): a card that is whole in the pane at a readable zoom is
 * selected and pulses and nothing else; any other is framed (fitView on the one card, up to 120 %). The canvas's own first fit comes first: a reveal is judged once it is done, so that what a reveal sees is the drawing
 * as the canvas framed it (Show Calls on a small drawing, and the cards that open under a card are then in view). The pane can change size just after (a pick from the search opens the docs tab beside the canvas):
 * the same test is made then, and a card still on its way is framed again for the new size.
 */
function CenterOn({ laid, context }: { laid: Layout; context: string }) {
  const { fitView } = useReactFlow();
  const store = useStoreApi();
  // the session's request to bring a node into view; done, it says so and the session clears it (a canvas opened later does not repeat it)
  const reveal = useSession((s) => s.reveal);
  // a canvas that shows another context leaves the request for that context's own canvas
  const target = useMemo(() => (reveal && reveal.context === context ? { id: reveal.id, n: reveal.n } : null), [reveal, context]);
  /** the last reveal that was judged: which card, when, and whether the camera is still on its way to it */
  const judged = useRef<{ id: string; at: number; moving: boolean } | null>(null);
  const width = useStore((s) => s.width);
  const height = useStore((s) => s.height);
  /** the card, the pane and the viewport as they are now: does the camera have to move? A card that is not known is framed. */
  const hasToMove = (id: string): boolean => {
    const { nodeLookup, width, height, transform } = store.getState();
    const node = nodeLookup.get(id);
    if (!node) return true;
    const box = { x: node.internals.positionAbsolute.x, y: node.internals.positionAbsolute.y, width: node.measured.width ?? node.width ?? CARD_W, height: node.measured.height ?? node.height ?? 0 };
    return needsFraming(box, { width, height }, { x: transform[0], y: transform[1], zoom: transform[2] });
  };
  const frame = (id: string, now: { id: string; at: number; moving: boolean }) => {
    now.moving = true;
    void fitView({ nodes: [{ id }], duration: motionMs(300, prefersReducedMotion()), maxZoom: 1.2 }).finally(() => {
      now.moving = false;
    });
  };
  useEffect(() => {
    if (!target || !laid.nodes.some((p) => p.id === target.id)) return;
    let stop = () => {};
    const done = () => {
      const now = { id: target.id, at: performance.now(), moving: false };
      judged.current = now;
      if (hasToMove(target.id)) frame(target.id, now);
      dispatch({ type: "revealed", n: target.n });
      pulse(target.id); // the card pulses for a second so the eye finds it
    };
    // the cards of a new layout are measured on the next frames; judging them before that judges nothing
    const timer = setTimeout(() => {
      if (!store.getState().fitViewQueued) return done();
      // the canvas's own first fit is still to come: the reveal is judged when it is done (a `fitView()` now would take its place)
      stop = store.subscribe((s) => {
        if (s.fitViewQueued) return;
        stop();
        done();
      });
    }, 60);
    return () => {
      clearTimeout(timer);
      stop();
    };
  }, [fitView, store, target, laid]);
  useEffect(() => {
    const last = judged.current;
    if (!last || !followsReveal(last.at, performance.now())) return;
    // a camera that is still on its way has no viewport to judge by: the card is framed again for the new size; one that has stopped is judged as it stands
    if (last.moving || hasToMove(last.id)) frame(last.id, last);
  }, [fitView, store, width, height]);
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
      const visible = new Set(onScreenIds(laid.nodes.map((p) => ({ id: p.id, x: p.x, y: p.y, w: p.width, h: cardHeight(p) })), { x, y, zoom }, { width, height }));
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

/** Menu commands (and the F key) that change only what this canvas draws: show or hide a card's code, fit the view, "Edit in Card" (the card shows its code and its editor takes the keyboard). */
function ViewCommands({ laid }: { laid: Layout }) {
  const { fitView } = useReactFlow();
  const cards = useRef(laid.nodes); // the latest layout, without listening again each time it changes
  cards.current = laid.nodes;
  useEffect(() => {
    const fit = () => void fitView({ duration: motionMs(300, prefersReducedMotion()) });
    const stop = on("viewCommand", ({ name, key, text }) => {
      if (name === "fitView") fit();
      else if (name === "checkKeys") post({ type: "keysChecked", ...checkKeys() });
      else if (key && (name === "edit" || name === "setDraft" || name === "save" || name === "saveFile")) {
        const card = cards.current.find((p) => p.id === key);
        if (!card) return;
        // "Edit in Card" (9.7): the code is shown, and its editor takes the keyboard, so typing starts the draft
        if (name === "edit" && card.key !== null && card.range !== null) {
          setCodeOpen(card.id, true);
          requestCodeFocus(card.id);
        }
        // the suite cannot type or press keys in a webview: these two are the draft's text and Save, as a person's typing and ⌘S would make them
        else if (name === "setDraft" && card.key !== null && text !== undefined) typeIn(card.id, card.key, text);
        else if (name === "save" && card.key !== null) void saveDraft(card.key);
        else if (name === "saveFile") void saveFile(cardRef(card)); // the Save button of "Save <file> to edit here"
      } else if (key && (name === "showCode" || name === "hideCode")) setCodeOpen(key, name === "showCode");
    });
    window.addEventListener(FIT_VIEW_EVENT, fit); // the F key (canvas/keys.ts)
    return () => {
      stop();
      window.removeEventListener(FIT_VIEW_EVENT, fit);
    };
  }, [fitView]);
  return null;
}

/**
 * Module-level, so it is the same object on every render. React Flow copies `fitViewOptions` into its store whenever the prop is a new object, and a queued `fitView()` (a reveal's, on one card) runs with the options
 * the store holds when the next frame resolves it: a literal in the JSX made the render that a reveal causes replace the card's options with these, and the whole drawing was fitted at once (a pick from the search).
 */
const FIT_OPTIONS = { maxZoom: 1 }; // a context of one card (a new EXPLORE) would otherwise open at React Flow's limit, 200 %

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
  useSearchDim(laid.nodes);
  const dimmed = useView((v) => v.dimmed);
  const edges = useMemo(() => {
    const onPath = root && drawnRoot ? pathEdges(root, drawnRoot, selected) : new Set<string>();
    // a call edge is dashed (03 §6); the way down to the selection is thicker, whichever kind the edge is
    return laid.edges.map((e) => ({ id: e.id, source: e.source, target: e.target, className: [e.kind === "call" && "call", onPath.has(e.id) && "on-path", dimmed?.has(e.target) && "searched-out"].filter(Boolean).join(" ") || undefined }));
  }, [laid, root, drawnRoot, selected, dimmed]);

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
      fitViewOptions={FIT_OPTIONS}
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
