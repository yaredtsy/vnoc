import { useEffect, useRef, useState } from "react";

import { BarButton } from "@/components/BarButton.tsx";
import { Codicon } from "@/components/Codicon.tsx";
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu.tsx";
import { agentWords, stoppable } from "@vnoc/shared/tourWriting.ts";
import type { Place } from "@vnoc/shared/tours.ts";
import { CrumbButton } from "../canvas/top-bar/CrumbButton.tsx";
import { FIT_VIEW_EVENT } from "../canvas/keys.ts";
import { on, post, request } from "../host.ts";
import { dispatch } from "../state/session.ts";
import { useData } from "../state/data.ts";
import { setHover, setMinimap, useView } from "../state/view.ts";
import { MOVED_MS, canGoBack, crumbsOf, movedByAgent } from "./bar.ts";
import { CENTER_TOUR_EVENT } from "./camera.ts";
import { goToStop } from "./goToCode.ts";
import { currentOf } from "./layout.ts";
import { go, halt, useTour } from "./store.ts";

/**
 * The bar's tour half (docs/26-tours/design/04-route-and-map.md §5), drawn in place of MAIN's focus crumbs while a tour is the active context: `←` back to where you were
 * (the core's `back`), the crumbs (the stops from the first to the current one: a click jumps, the pointer over one points at its card), `moved by the agent` for three
 * seconds when another process moved the place, `⌖` (centre the current card) and `⋯` (Fit Route, Show Minimap, Start Again, Open Tour…, Reveal Root in Main, Go to Code, Close
 * Tour). The chip is the context chips'. Draws the view the host holds.
 */
export function TourBar({ tourId, contextId, width }: { tourId: string; contextId: string; width: number }) {
  const held = useTour();
  const view = held.id === tourId && held.view && "stops" in held.view ? held.view : null;
  const root = useData((s) => s.structure?.root ?? null);
  const [moved, setMoved] = useState(false);
  const before = useRef<Place | null>(null);
  const place = view?.place ?? null;
  // the agent: `writing 6/10 ■`, `stopping`, `stopped`, `no news`; nothing for a finished tour (design 04 §5 #4). The ■ is the only button here
  const agent = view ? agentWords(view) : null;
  const canStop = view ? stoppable(view) : false;

  // a place that moved by a command shows `moved by the agent` for three seconds; the first place seen is nothing to say
  useEffect(() => {
    if (!place) return;
    const was = before.current;
    before.current = place;
    if (!movedByAgent(was, place)) return;
    setMoved(true);
    const timer = setTimeout(() => setMoved(false), MOVED_MS);
    return () => clearTimeout(timer);
  }, [place?.at, place?.moved]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    before.current = null;
    setMoved(false);
  }, [tourId]);

  const crumbs = view ? crumbsOf(view) : [];
  const shown = width < 640 ? crumbs.slice(-1) : crumbs; // narrow: only the last stop (design 04 §5)
  // what the suite reads of the bar
  useEffect(() => {
    post({ type: "tourBar", crumbs: crumbs.map((c) => c.label), moved, back: view ? canGoBack(view) : false, agent, stop: canStop });
  }, [crumbs.map((c) => c.id).join(","), moved, view?.place.back.length, agent, canStop]); // eslint-disable-line react-hooks/exhaustive-deps

  const current = view ? currentOf(view) : null;
  const act = {
    menuStartAgain: () => go(tourId, "cover"),
    menuOpenTour: () => void request("openTour").catch(() => {}),
    menuClose: () => dispatch({ type: "closeContext", id: contextId }, contextId),
    // the ■ on the bar, as the suite presses it
    menuStop: () => canStop && halt(tourId),
    menuGoToCode: () => current?.stop && goToStop(root, current.stop, current.step),
    menuRevealRoot: () => {
      const key = view?.stops[0]?.key;
      if (!key) return;
      dispatch({ type: "reveal", id: key, canvas: true }, "main"); // MAIN selects it and brings it into view,
      dispatch({ type: "activate", id: "main" }, "main"); // then MAIN is the one the canvas draws. The tour stays where it was
    },
  };
  // the extension's test suite cannot open a menu or click a menu entry: it sends the entry's name
  const latest = useRef(act);
  latest.current = act;
  useEffect(
    () =>
      on("viewCommand", ({ name, key }) => {
        if (name === "stageClick" && key && key in latest.current) (latest.current as Record<string, () => void>)[key]();
      }),
    [],
  );

  return (
    <>
      {/* the `←` is before the chips (`TourBack`) and the chip is ContextChips': here, after them, are the crumbs */}
      <nav aria-label="Tour" className="flex min-w-0 flex-1 items-center overflow-hidden" data-part="tour-crumbs">
        {shown.map((c, i) => (
          <span key={c.id} className="flex items-center">
            {i > 0 && <Codicon name="chevron-right" className="flex-none text-[16px] text-muted-foreground" />}
            <CrumbButton label={c.label} title={`Stop ${c.id}`} current={i === shown.length - 1} onHover={(on) => setHover(on ? c.id : null)} onClick={() => go(tourId, c.id)} />
          </span>
        ))}
        {crumbs.length === 0 && view && <span className="px-1.5 text-[length:var(--font-size)] font-semibold">{view.title}</span>}
      </nav>
      {moved && (
        <span className="flex-none text-[11px] whitespace-nowrap text-muted-foreground" role="status" data-part="moved">
          moved by the agent
        </span>
      )}
      {agent && (
        <span className="flex flex-none items-center gap-1 text-[11px] whitespace-nowrap text-muted-foreground" role="status" data-part="agent">
          {agent}
          {canStop && (
            <button type="button" data-part="stop" aria-label="Stop writing" title="Stop the agent: it finishes the stop it is on" className="flex size-5 items-center justify-center rounded-sm text-foreground hover:bg-toolbar-hover" onClick={() => halt(tourId)}>
              ■
            </button>
          )}
        </span>
      )}
    </>
  );
}

/** The bar's `←` in a tour context: back to where you were before the last jump (the core's `back`, 20 at most; Next and Prev never add to it). Disabled with nothing to go back to. */
export function TourBack({ tourId }: { tourId: string }) {
  const held = useTour();
  const view = held.id === tourId && held.view && "stops" in held.view ? held.view : null;
  return <BarButton icon="arrow-left" label="Back to where you were" keys="Alt+←" disabled={!view || !canGoBack(view)} onClick={() => go(tourId, "back")} />;
}

/** The bar's `⌖` and `⋯` in a tour context (the docs toggle and the ⚠ are the bar's own). */
export function TourBarButtons({ tourId, contextId }: { tourId: string; contextId: string }) {
  const held = useTour();
  const view = held.id === tourId && held.view && "stops" in held.view ? held.view : null;
  const root = useData((s) => s.structure?.root ?? null);
  const minimap = useView((v) => v.minimap);
  const current = view ? currentOf(view) : null;
  return (
    <>
      <BarButton icon="target" label="Centre the current stop" disabled={!view || current?.stop == null} onClick={() => window.dispatchEvent(new Event(CENTER_TOUR_EVENT))} />
      <DropdownMenu>
        <DropdownMenuTrigger render={<BarButton icon="ellipsis" label="More" />} />
        <DropdownMenuContent>
          <DropdownMenuItem onClick={() => window.dispatchEvent(new Event(FIT_VIEW_EVENT))}>Fit Route</DropdownMenuItem>
          <DropdownMenuCheckboxItem checked={minimap} onCheckedChange={setMinimap}>
            Show Minimap
          </DropdownMenuCheckboxItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={() => go(tourId, "cover")}>Start Again</DropdownMenuItem>
          <DropdownMenuItem onClick={() => void request("openTour").catch(() => {})}>Open Tour…</DropdownMenuItem>
          <DropdownMenuItem
            disabled={!view?.stops[0]?.key}
            onClick={() => {
              const key = view?.stops[0]?.key;
              if (!key) return;
              dispatch({ type: "reveal", id: key, canvas: true }, "main");
              dispatch({ type: "activate", id: "main" }, "main");
            }}
          >
            Reveal Root in Main
          </DropdownMenuItem>
          <DropdownMenuItem disabled={!current?.stop} onClick={() => current?.stop && goToStop(root, current.stop, current.step)}>
            Go to Code
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={() => dispatch({ type: "closeContext", id: contextId }, contextId)}>Close Tour</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  );
}
