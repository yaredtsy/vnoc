import { useEffect, useMemo, useRef, useState } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { cn } from "@/lib/utils.ts";
import { footWords, stoppable } from "@vnoc/shared/tourWriting.ts";
import type { TourRow, TourView } from "@vnoc/shared/tours.ts";
import { on, post, request } from "../../host.ts";
import { dispatch, useSession } from "../../state/session.ts";
import { setToursFolded, setToursShare, useView } from "../../state/view.ts";
import { goToStop } from "../../tour/goToCode.ts";
import { go, halt, loadTour, useTour } from "../../tour/store.ts";
import { Splitter } from "../Splitter.tsx";
import { appearedFromOutside } from "./appear.ts";
import { DEFAULT_TOURS_SHARE, dragTours, toursShare } from "./split.ts";
import { useData } from "../../state/data.ts";
import { listItem } from "./list.ts";
import { headerPlace, rowKey, routeRows } from "./route.ts";
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
  const share = useView((v) => v.toursShare);
  const tour = useSession((s) => s.order.map((id) => s.contexts[id]).find((c) => c.kind === "tour") ?? null);
  const tourId = tour?.tour ?? null;
  const tourActive = useSession((s) => tour !== null && s.active === tour.id);
  const held = useTour();
  const view: TourView | null = tourId !== null && held.id === tourId && held.view && "stops" in held.view ? held.view : null;
  const [showList, setShowList] = useState(false);
  const [cursor, setCursor] = useState(-1);
  const box = useRef<HTMLDivElement>(null);
  const section = useRef<HTMLElement>(null);
  const dragFrom = useRef(0);

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

  // a tour that appeared from outside (the agent started one) while this window has none open unfolds the section: the list shows it
  const listed = useRef<string[] | null>(null);
  useEffect(() => {
    if (rows === null) return;
    const now = rows.map((r) => r.id);
    if (appearedFromOutside(listed.current, now, tourId)) setToursFolded(false);
    listed.current = now;
  }, [rows]); // eslint-disable-line react-hooks/exhaustive-deps

  const mode: "list" | "route" = tourId !== null && !showList ? "route" : "list";
  // a row says where you are (`words`), and moving in a tour does not send `toursChanged`: the list is asked for again whenever it is shown
  useEffect(() => {
    if (mode === "list" && !folded) void request("tours", undefined).then((r) => useTourList.setState({ rows: r }), () => {});
  }, [mode, folded]);
  const route = useMemo(() => (view ? routeRows(view) : []), [view]);
  // the route's foot (design 05 §1): what the agent is doing, with ■ Stop while it writes; and, for a tour with no plan, the one row under the last stop that says another may come
  const foot = mode === "route" && view ? footWords(view) : null;
  const canStop = view ? stoppable(view) : false;
  const more = mode === "route" && view !== null && view.state === "writing" && view.of === null;
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
  const stopWriting = () => tourId !== null && canStop && halt(tourId);
  const latest = useRef({ openTour, jump, closeTour, key, rows, route, stopWriting });
  latest.current = { openTour, jump, closeTour, key, rows, route, stopWriting };
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
        else if (what === "stop") l.stopWriting();
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
      foot: foot === null ? null : { text: foot, stop: canStop },
      more,
      route: route.map((r) => ({ id: r.id, kind: r.type, text: r.type === "stop" ? r.name : r.text, mark: r.type === "stop" ? r.mark : null, current: r.current, number: r.type === "stop" ? r.number : null, description: r.type === "stop" ? r.description : null, struck: r.type === "stop" && r.struck, depth: r.depth })),
    });
  }, [rows, route, folded, mode, count, tourId, view?.title, foot, canStop, more]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <section
      ref={section}
      aria-label="Tours"
      data-section="tours"
      className={cn("relative flex min-h-0 flex-none flex-col border-t border-t-border", !folded && "min-h-[112px] max-h-[calc(100%-112px)]")}
      style={folded ? undefined : { height: `${share * 100}%` }}
      // a press anywhere in the route while another context is active makes the tour's context the active one (04 §1.3)
      onPointerDown={() => tour && !tourActive && mode === "route" && dispatch({ type: "activate", id: tour.id }, tour.id)}
    >
      {/* the splitter between the tree and the section: drag it, double-click for the default; its size is kept per window */}
      {!folded && (
        <Splitter
          onStart={() => (dragFrom.current = section.current?.offsetHeight ?? 0)}
          onMove={(dy) => {
            const total = section.current?.parentElement?.clientHeight ?? 0;
            if (section.current && total > 0) section.current.style.height = `${dragTours(total, dragFrom.current, dy)}px`;
          }}
          onEnd={(dy) => {
            const total = section.current?.parentElement?.clientHeight ?? 0;
            if (total > 0) setToursShare(toursShare(total, dragTours(total, dragFrom.current, dy)));
          }}
          onReset={() => setToursShare(DEFAULT_TOURS_SHARE)}
        />
      )}
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
              <>
                {route.map((r, i) => <RouteRowView key={rowKey(r)} row={r} domId={`tours-route-${i}`} focused={cursor === i} onClick={() => jump(r.id, r.current)} onGoToCode={() => goToCode(r.id)} />)}
                {more && (
                  <div className="flex h-[22px] flex-none items-center gap-1.5 pl-2 text-[13px] text-muted-foreground" data-part="route-more" aria-label="The agent may add more stops">
                    <span className="w-3 flex-none text-center text-[11px]">◌</span>
                    <span className="w-5 flex-none" />
                    <span>…</span>
                  </div>
                )}
              </>
            ) : (
              <p className="px-3 py-2 text-[12px] text-muted-foreground">Opening the tour…</p>
            )}
          </div>
          {foot !== null && view && (
            <div className="flex h-[22px] flex-none items-center gap-1 border-t border-t-border px-2 text-[11px] text-muted-foreground" data-part="route-foot" role="status">
              <span className="min-w-0 flex-1 truncate">{foot}</span>
              {canStop && (
                <button type="button" data-part="stop" className="flex flex-none items-center gap-1 rounded-sm px-1 text-foreground hover:bg-toolbar-hover" title="Stop the agent: it finishes the stop it is on" onClick={stopWriting}>
                  ■ Stop
                </button>
              )}
            </div>
          )}
        </>
      )}
    </section>
  );
}
