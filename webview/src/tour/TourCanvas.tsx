import { Background, Controls, type Edge, type Node, ReactFlow, useReactFlow, useStore, useStoreApi } from "@xyflow/react";
import { useEffect, useMemo, useRef } from "react";

import { FIT_VIEW_EVENT } from "../canvas/keys.ts";
import { CARD_H, CARD_W } from "../canvas/layout.ts";
import { motionMs, prefersReducedMotion } from "../lib/motion.ts";
import { on, post } from "../host.ts";
import { rememberViewport, reportViewport, savedViewport } from "../state/viewports.ts";
import { CENTER_TOUR_EVENT, cameraMove, centre } from "./camera.ts";
import { mapOf, boxOf, type TourCardData } from "./map.ts";
import { go, loadTour, useTour } from "./store.ts";
import { TourCard } from "./TourCard.tsx";

const nodeTypes = { tourCard: TourCard };
const FIT_OPTIONS = { maxZoom: 1 };

/**
 * The camera of design 04 §4, inside `<ReactFlow>`: the current card is in view after every step and nothing else is promised. A new stop moves the map by the least that
 * shows its card (`cameraMove`); a step in the same stop does not move it; the cover and the end card fit the whole route; `⌖` centres the card; a pane that got
 * smaller or taller pans only if the card is now covered. A person's own pan is theirs until the next stop that is out of view.
 */
function TourCamera({ cards, stop, at, tourId }: { cards: TourCardData[]; stop: string | null; at: "cover" | "end" | "step"; tourId: string }) {
  const { setViewport, fitView } = useReactFlow();
  const store = useStoreApi();
  const width = useStore((s) => s.width);
  const height = useStore((s) => s.height);
  const previous = useRef<{ stop: string | null; at: string } | null>(null);
  const duration = () => motionMs(200, prefersReducedMotion());
  const latest = useRef({ cards, stop });
  latest.current = { cards, stop };

  const showCurrent = (moveAlways: boolean) => {
    const card = latest.current.cards.find((c) => c.id === latest.current.stop);
    if (!card) return;
    const { transform, width, height } = store.getState();
    const view = { x: transform[0], y: transform[1], zoom: transform[2] };
    const move = moveAlways ? centre(boxOf(card), view.zoom, { width, height }) : cameraMove(boxOf(card), view, { width, height });
    if (move) void setViewport(move, { duration: duration() });
  };

  useEffect(() => {
    const before = previous.current;
    previous.current = { stop, at };
    if (before === null) return; // the first drawing: the canvas fitted it, or restored the viewport this context had
    if (at !== "step") {
      if (before.at !== at) void fitView({ duration: duration(), maxZoom: 1 }); // the cover and the end card: the whole route
      return;
    }
    if (before.stop !== stop) showCurrent(false);
  }, [stop, at]); // eslint-disable-line react-hooks/exhaustive-deps
  // the pane changed size (the stage was dragged, folded, or unfolded): pan only if the current card is now covered
  useEffect(() => {
    if (previous.current && previous.current.at === "step") showCurrent(false);
  }, [width, height]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    const centreIt = () => showCurrent(true);
    const fit = () => void fitView({ duration: duration(), maxZoom: 1 });
    window.addEventListener(CENTER_TOUR_EVENT, centreIt);
    window.addEventListener(FIT_VIEW_EVENT, fit);
    // the extension's test suite cannot drag the map: it sets the viewport as a person's pan and zoom would
    const stopView = on("viewCommand", ({ name, text, key }) => {
      if (name === "mapClick" && key) return void (latest.current.stop !== key && go(tourId, key)); // a click on a card, as the suite sends it
      if (name !== "setView" || !text) return;
      const [x, y, zoom] = text.split(",").map(Number);
      void setViewport({ x, y, zoom }, { duration: 0 });
    });
    return () => {
      window.removeEventListener(CENTER_TOUR_EVENT, centreIt);
      window.removeEventListener(FIT_VIEW_EVENT, fit);
      stopView();
    };
  }, [fitView, setViewport, tourId]); // eslint-disable-line react-hooks/exhaustive-deps
  return null;
}

/**
 * The map while the active context is a tour (design 04 §2): one card for every stop of the route, from the first second, the ring on the current one, the edges
 * between them (dashed for a call, solid for containment) and the heavy ones from the first stop to the current one. A click on a card is a jump to the stop. The
 * route is fixed (the core chose it): no chevrons, no Focus, no Explore. Draws the `TourView` and the place the host holds.
 */
export function TourCanvas({ tourId, contextId }: { tourId: string; contextId: string }) {
  const held = useTour();
  const view = held.id === tourId && held.view && "stops" in held.view ? held.view : null;
  useEffect(() => {
    let wanted = true;
    loadTour(tourId, () => wanted);
    return () => {
      wanted = false;
    };
  }, [tourId]);
  const map = useMemo(() => (view ? mapOf(view) : null), [view]);
  const nodes = useMemo<Node<TourCardData>[]>(() => (map?.cards ?? []).map((c) => ({ id: c.id, type: "tourCard", position: { x: c.x, y: c.y }, data: c, width: CARD_W, height: CARD_H })), [map]);
  const edges = useMemo<Edge[]>(() => (map?.edges ?? []).map((e) => ({ id: e.id, source: e.source, target: e.target, className: [e.kind === "call" && "call", e.onPath && "on-path"].filter(Boolean).join(" ") || undefined })), [map]);
  const saved = useMemo(() => savedViewport(contextId, tourId), [contextId, tourId]);
  const at = view ? (view.place.at === "cover" || view.place.at === "end" ? view.place.at : "step") : "cover";

  // what the suite reads of what was drawn
  useEffect(() => {
    if (!map) return;
    post({ type: "mapDrawn", tour: tourId, cards: map.cards.map((c) => ({ id: c.id, number: c.number, mark: c.mark, here: c.here, callLine: c.callLine, description: c.description, planned: c.planned })), heavy: map.edges.filter((e) => e.onPath).map((e) => e.id), edges: map.edges.map((e) => `${e.id}:${e.kind}`) });
  }, [map, tourId]);

  if (!view || !map) return <div className="flex min-w-0 flex-1 items-center justify-center text-[12px] text-muted-foreground">Opening the tour…</div>;
  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      onNodeClick={(_, node) => view.place.stop !== node.id && go(tourId, node.id)}
      onlyRenderVisibleElements
      elementsSelectable={false}
      nodesDraggable={false}
      nodesConnectable={false}
      zoomOnDoubleClick={false}
      fitView={saved === null}
      fitViewOptions={FIT_OPTIONS}
      defaultViewport={saved ?? undefined}
      onInit={(rf) => reportViewport(contextId, rf.getViewport())}
      onMoveEnd={(_, viewport) => rememberViewport(contextId, tourId, viewport)}
      className="min-w-0 flex-1"
      minZoom={0.02}
      proOptions={{ hideAttribution: true }}
    >
      <Background />
      <Controls showInteractive={false} />
      <TourCamera cards={map.cards} stop={view.place.stop} at={at} tourId={tourId} />
    </ReactFlow>
  );
}
