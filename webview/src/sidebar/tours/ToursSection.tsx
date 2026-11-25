import { useEffect, useMemo, useRef, useState } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { cn } from "@/lib/utils.ts";
import type { TourRow, TourView } from "@vnoc/shared/tours.ts";
import { on, post, request } from "../../host.ts";
import { dispatch, useSession } from "../../state/session.ts";
import { setToursFolded, useView } from "../../state/view.ts";
import { goToStop } from "../../tour/goToCode.ts";
import { go, loadTour, useTour } from "../../tour/store.ts";
import { useData } from "../../state/data.ts";
import { listItem } from "./list.ts";
import { headerPlace, routeRows } from "./route.ts";
import { tourKey } from "./keys.ts";
import { useTourList } from "./store.ts";
import { RouteRowView, TourListRow } from "./TourRows.tsx";

/**
 * The TOURS section (docs/26-tours/design/07-tour-list.md §2, 04 §1): under the context panels. Folded it says its name and how many tours there are; open it is
 * the list of tours (the **All** view), or, while a tour is open, that tour's route, with `‹ Tours` to go back to the list (the tour stays open). It draws the
 * core's `TourRow`s and the open tour's `TourView` as they come, and every click is a request: open a tour (`tourOpen`), jump to a stop or a step (`tourGo`).
 */
export function ToursSection() {
  const rows = useTourList((s) => s.rows);
  const folded = useView((v) => v.toursFolded);
  const tour = useSession((s) => s.order.map((id) => s.contexts[id]).find((c) => c.kind === "tour") ?? null);
  const tourId = tour?.tour ?? null;
  const tourActive = useSession((s) => tour !== null && s.active === tour.id);
  const held = useTour();
  const view: TourView | null = tourId !== null && held.id === tourId && held.view && "stops" in held.view ? held.view : null;
  const [showList, setShowList] = useState(false);
  const [cursor, setCursor] = useState(-1);
  const box = useRef<HTMLDivElement>(null);

  // the sidebar's own copy of the open tour: the host answers from what it holds, and sends it again whenever it changes
  useEffect(() => {
    if (tourId === null) return;
    let wanted = true;
    loadTour(tourId, () => wanted);
    return () => {
      wanted = false;
    };
  }, [tourId]);

  // a tour that is opened (not one the window restored) unfolds the section and shows its route
  const seen = useRef(tourId);
  useEffect(() => {
    if (tourId !== seen.current && tourId !== null) {
      setToursFolded(false);
      setShowList(false);
    }
    seen.current = tourId;
  }, [tourId]);

  const mode: "list" | "route" = tourId !== null && !showList ? "route" : "list";
  // a row says where you are (`words`), and moving in a tour does not send `toursChanged`: the list is asked for again whenever it is shown
  useEffect(() => {
    if (mode === "list" && !folded) void request("tours", undefined).then((r) => useTourList.setState({ rows: r }), () => {});
  }, [mode, folded]);
  const route = useMemo(() => (view ? routeRows(view) : []), [view]);
  const count = mode === "route" ? route.length : (rows?.length ?? 0);

  const openTour = (row: TourRow) => {
    setShowList(false);
    dispatch({ type: "tourOpen", id: row.id, title: row.title });
  };
  const root = useData((s) => s.structure?.root ?? null);
  // Go to Code for a stop of the route: the real file at the stop's first line (the hover `↗`, and a double-click on the row)
  const goToCode = (id: string) => {
    const stop = view?.stops.find((s) => s.id === id);
    if (stop) goToStop(root, stop);
  };
  const jump = (id: string, current: boolean) => tourId !== null && !current && go(tourId, id);
  const closeTour = () => tour && dispatch({ type: "closeContext", id: tour.id }, tour.id);
  // the row's action, for a click and for Enter: one function
  const act = (index: number) => {
    if (mode === "list") {
      const row = rows?.[index];
      if (row && listItem(row).clickable) openTour(row);
    } else {
      const r = route[index];
      if (r) jump(r.id, r.current);
    }
  };

  // the current stop is where the focus starts, and its row stays in view
  const currentAt = route.findIndex((r) => r.type === "stop" && r.current);
  useEffect(() => {
    if (mode === "route") setCursor(currentAt);
    else setCursor(-1);
  }, [mode, tourId, currentAt]);
  useEffect(() => {
    box.current?.querySelector<HTMLElement>(mode === "route" ? "[aria-current=step][data-stop]" : "[aria-selected=true]")?.scrollIntoView({ block: "nearest" });
  }, [mode, currentAt, folded]);

  const key = (k: string) => {
    const move = tourKey(k, cursor, mode === "list" ? (rows?.length ?? 0) : route.length, mode);
    if (!move) return false;
    if (move.type === "focus") setCursor(move.index);
    else if (move.type === "open") act(cursor);
    else setShowList(true);
    return true;
  };

  // the extension's test suite cannot click in a webview: it sends the click
  const latest = useRef({ openTour, jump, closeTour, key, rows, route });
  latest.current = { openTour, jump, closeTour, key, rows, route };
  useEffect(
    () =>
      on("viewCommand", ({ name, key: what }) => {
        if (name !== "toursClick" || !what) return;
        const l = latest.current;
        const [kind, id] = [what.split(":")[0], what.slice(what.indexOf(":") + 1)];
        if (what === "fold") setToursFolded(true);
        else if (what === "unfold") setToursFolded(false);
        else if (what === "back") setShowList(true);
        else if (what === "close") l.closeTour();
        else if (kind === "row") {
          const row = l.rows?.find((r) => r.id === id);
          if (row) l.openTour(row);
        } else if (kind === "stop" || kind === "step") {
          const r = l.route.find((x) => x.id === id);
          if (r) l.jump(r.id, r.current);
        } else if (kind === "key") box.current?.focus(), l.key(id);
      }),
    [],
  );

  // what the suite reads of what was drawn
  useEffect(() => {
    post({
      type: "toursDrawn",
      folded,
      mode,
      count,
      header: view && mode === "route" ? `‹ Tours ▶ ${view.title} ${headerPlace(view)}`.trim() : "TOURS",
      list: (rows ?? []).map((r) => {
        const item = listItem(r);
        return { id: item.id, title: item.title, words: item.words, question: item.question, active: r.id === tourId && !r.private_copy, clickable: item.clickable };
      }),
      route: route.map((r) => ({ id: r.id, kind: r.type, text: r.type === "stop" ? r.name : r.text, mark: r.type === "stop" ? r.mark : null, current: r.current, number: r.type === "stop" ? r.number : null, description: r.type === "stop" ? r.description : null, depth: r.depth })),
    });
  }, [rows, route, folded, mode, count, tourId, view?.title]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <section
      aria-label="Tours"
      data-section="tours"
      className={cn("relative flex min-h-0 flex-none flex-col border-t border-t-border", !folded && "min-h-[112px]")}
      style={folded ? undefined : { height: "45%" }}
      // a press anywhere in the route while another context is active makes the tour's context the active one (04 §1.3)
      onPointerDown={() => tour && !tourActive && mode === "route" && dispatch({ type: "activate", id: tour.id }, tour.id)}
    >
      <div className="flex h-[22px] flex-none items-center gap-1 bg-[color:var(--section-header-bg)] pr-1 pl-1 text-[11px] font-bold tracking-wide text-[color:var(--section-header-fg)] uppercase">
        <button type="button" className="flex min-w-0 flex-1 items-center gap-0.5 text-left uppercase" aria-expanded={!folded} onClick={() => setToursFolded(!folded)}>
          <Codicon name={folded ? "chevron-right" : "chevron-down"} className="text-[16px]" />
          <span>Tours</span>
          {rows !== null && <span className="ml-1.5 rounded-full bg-[color:var(--card-badge-bg)] px-1.5 text-[10px] font-normal text-[color:var(--card-badge-fg)] normal-case">{rows.length}</span>}
        </button>
      </div>
      {!folded && (
        <>
          {mode === "route" && view && (
            <div className={cn("flex h-[22px] flex-none items-center gap-1 border-l-2 pr-1 pl-1 text-[12px]", tourActive ? "border-l-[color:var(--ring)]" : "border-l-transparent")} data-part="route-header">
              <button type="button" className="flex flex-none items-center text-muted-foreground hover:text-foreground" title="Back to the list of tours (the tour stays open)" onClick={() => setShowList(true)}>
                <Codicon name="chevron-left" className="text-[14px]" />
                <span>Tours</span>
              </button>
              <span className="min-w-0 flex-1 truncate font-bold">▶ {view.title}</span>
              <span className="flex-none text-[11px] text-muted-foreground">{headerPlace(view)}</span>
              <button type="button" aria-label="Close Tour" title="Close Tour" className="flex size-4 flex-none items-center justify-center rounded-sm hover:bg-toolbar-hover" onClick={closeTour}>
                <Codicon name="close" className="text-[14px]" />
              </button>
            </div>
          )}
          <div
            ref={box}
            role="listbox"
            tabIndex={0}
            aria-label={mode === "route" ? "Stops of the tour" : "Tours"}
            aria-activedescendant={cursor >= 0 ? `tours-${mode}-${cursor}` : undefined}
            className="group min-h-0 flex-1 overflow-y-auto outline-none"
            onKeyDown={(e) => {
              if (e.altKey || e.ctrlKey || e.metaKey) return;
              if (key(e.key)) {
                e.preventDefault();
                e.stopPropagation();
              }
            }}
          >
            {mode === "list" ? (
              rows === null ? null : rows.length === 0 ? (
                <p className="px-3 py-2 text-[12px] text-muted-foreground">No tours yet. A tour is your agent explaining a piece of the code, step by step.</p>
              ) : (
                rows.map((row, i) => <TourListRow key={`${row.id}:${row.folder}`} row={row} domId={`tours-list-${i}`} open={tourId} focused={cursor === i} onOpen={() => openTour(row)} />)
              )
            ) : view ? (
              route.map((r, i) => <RouteRowView key={r.id} row={r} domId={`tours-route-${i}`} focused={cursor === i} onClick={() => jump(r.id, r.current)} onGoToCode={() => goToCode(r.id)} />)
            ) : (
              <p className="px-3 py-2 text-[12px] text-muted-foreground">Opening the tour…</p>
            )}
          </div>
        </>
      )}
    </section>
  );
}
