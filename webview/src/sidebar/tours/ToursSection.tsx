import { useEffect, useMemo, useRef, useState } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { cn } from "@/lib/utils.ts";
import { callFacts } from "@vnoc/shared/calls.ts";
import { findNode } from "@vnoc/shared/tree.ts";
import { footWords, stoppable } from "@vnoc/shared/tourWriting.ts";
import type { TourRow, TourView } from "@vnoc/shared/tours.ts";
import { on, post, request } from "../../host.ts";
import { useCalls } from "../../state/calls.ts";
import { dispatch, useSession } from "../../state/session.ts";
import { setToursFolded, setToursShare, useView } from "../../state/view.ts";
import { goToStop } from "../../tour/goToCode.ts";
import { go, goThen, halt, loadTour, useTour } from "../../tour/store.ts";
import { Splitter } from "../Splitter.tsx";
import { appearedFromOutside } from "./appear.ts";
import { DEFAULT_TOURS_SHARE, dragTours, toursShare } from "./split.ts";
import { useData } from "../../state/data.ts";
import { listItem } from "./list.ts";
import { emptyOn, lineTwo, opensAt, selectionOf, showAll, stateWordOf, viewButtons, viewKey, viewOf, type ListView } from "./onView.ts";
import { headerPlace, rowKey, routeRows } from "./route.ts";
import { tourKey } from "./keys.ts";
import { useTourList } from "./store.ts";
import { RouteRowView, TourListRow } from "./TourRows.tsx";

/**
 * The TOURS section (docs/26-tours/design/07-tour-list.md §2, 04 §1): under the context panels. Folded it says its name and how many tours there are; open it is
 * the list of tours (the **All** view, or **On the selected node**: the tours through the active context's selection, each saying how), or, while a tour is open, that tour's route, with `‹ Tours` to go back to the list (the tour stays open). It draws the
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
  // the second view follows the active context's selection (a selected call is its callee); with none the list is All
  const saved = useSession((s) => s.tourList.view);
  const contexts = useSession((s) => s.contexts);
  const active = useSession((s) => s.active);
  const tree = useData((s) => s.structure?.root ?? null);
  const trees = useCalls((s) => s.trees);
  const selection = useMemo(() => selectionOf({ contexts, active }, tree, (call) => callFacts(trees, call)), [contexts, active, tree, trees]);
  const selectionName = selection !== null && tree ? (findNode(tree, selection)?.name ?? null) : null;
  const listView: ListView = viewOf(saved, selection);
  // the core's tours through the selection, asked when the selection changes and whenever the list does (a tour changed): plain first, then with the ids it answered, as the host's list does
  const [onRows, setOnRows] = useState<{ key: string; rows: TourRow[] } | null>(null);
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
  // asked only when the selection or what the list holds really changed (the ids and words of the rows, not the array: the list is drawn again for many reasons). In **All** the ask is plain
  // (the button's count); in the second view the rows are asked with `check` too, as the host asks the list
  const rowsSignature = rows === null ? null : rows.map((r) => `${r.id}/${r.folder}/${r.words}`).join("|");
  const asking = selection !== null && mode === "list" && !folded && rowsSignature !== null;
  const checking = listView === "on";
  useEffect(() => {
    if (!asking) return;
    let wanted = true;
    const ask = (check: string[]) => request("tours", check.length > 0 ? { on: selection, check } : { on: selection });
    ask([]).then(
      (first) => {
        if (!wanted) return;
        setOnRows((held) => (held?.key === selection && JSON.stringify(held.rows) === JSON.stringify(first) ? held : { key: selection, rows: first }));
        const ids = [...new Set(first.filter((r) => r.readable).map((r) => r.id))];
        if (checking && ids.length > 0) void ask(ids).then((checked) => wanted && setOnRows((held) => (held?.key === selection && JSON.stringify(held.rows) === JSON.stringify(checked) ? held : { key: selection, rows: checked })), () => {});
      },
      () => wanted && setOnRows(null),
    );
    return () => {
      wanted = false;
    };
  }, [asking, selection, rowsSignature, checking]); // eslint-disable-line react-hooks/exhaustive-deps
  const onShown = selection !== null && onRows?.key === selection ? onRows.rows : null;
  const shown = listView === "on" ? onShown : rows;
  const buttons = viewButtons(listView, rows?.length ?? 0, selectionName === null ? null : { name: selectionName }, onShown?.length ?? null);
  const emptyOnText = listView === "on" && onShown !== null && onShown.length === 0 && selectionName !== null ? emptyOn(selectionName) : null;
  const switchView = (to: ListView) => dispatch({ type: "tourView", view: to === "on" ? "onSelection" : "all" });
  const route = useMemo(() => (view ? routeRows(view) : []), [view]);
  // the route's foot (design 05 §1): what the agent is doing, with ■ Stop while it writes; and, for a tour with no plan, the one row under the last stop that says another may come
  const foot = mode === "route" && view ? footWords(view) : null;
  const canStop = view ? stoppable(view) : false;
  const more = mode === "route" && view !== null && view.state === "writing" && view.of === null;
  const count = mode === "route" ? route.length : (rows?.length ?? 0);

  const openTour = (row: TourRow) => {
    setShowList(false);
    const open = () => dispatch({ type: "tourOpen", id: row.id, title: row.title });
    // from the second view a tour opens where it is on the node: the cover for a tour that starts there, else at its stop (the core's `on.first`); from All, where it was left
    const at = opensAt(row, listView);
    if (at === null) open();
    else goThen(row.id, at, open);
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
      const row = shown?.[index];
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
    const move = tourKey(k, cursor, mode === "list" ? (shown?.length ?? 0) : route.length, mode);
    if (!move) return false;
    if (move.type === "focus") setCursor(move.index);
    else if (move.type === "open") act(cursor);
    else setShowList(true);
    return true;
  };

  // the extension's test suite cannot click in a webview: it sends the click
  const stopWriting = () => tourId !== null && canStop && halt(tourId);
  const latest = useRef({ openTour, jump, closeTour, key, rows: shown, route, stopWriting, switchView, listView, canOn: selection !== null });
  latest.current = { openTour, jump, closeTour, key, rows: shown, route, stopWriting, switchView, listView, canOn: selection !== null };
  useEffect(
    () =>
      on("viewCommand", ({ name, key: what }) => {
        // **Show Tours** and a click on `▶ N` (the host did the selecting and the switching): the section is shown, as the list
        if (name === "showTours") {
          setToursFolded(false);
          setShowList(true);
          return;
        }
        if (name !== "toursClick" || !what) return;
        const l = latest.current;
        const [kind, id] = [what.split(":")[0], what.slice(what.indexOf(":") + 1)];
        if (what === "fold") setToursFolded(true);
        else if (what === "unfold") setToursFolded(false);
        else if (what === "back") setShowList(true);
        else if (what === "close") l.closeTour();
        else if (what === "stop") l.stopWriting();
        else if (what === "view:all" || what === "view:on") l.switchView(what === "view:on" ? "on" : "all");
        else if (what === "showAll") l.switchView("all");
        else if (what.startsWith("viewkey:")) {
          const to = viewKey(what.slice("viewkey:".length), l.listView, l.canOn);
          if (to) l.switchView(to);
        }
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
      view: listView,
      buttons: buttons.map((b) => b.text),
      empty: emptyOnText,
      list: (shown ?? []).map((r) => {
        const item = listItem(r);
        return { id: item.id, title: item.title, words: item.words, line2: lineTwo(r, listView), stateWord: stateWordOf(r, listView), question: item.question, active: r.id === tourId && !r.private_copy, clickable: item.clickable };
      }),
      foot: foot === null ? null : { text: foot, stop: canStop },
      more,
      route: route.map((r) => ({ id: r.id, kind: r.type, text: r.type === "stop" ? r.name : r.text, mark: r.type === "stop" ? r.mark : null, current: r.current, number: r.type === "stop" ? r.number : null, description: r.type === "stop" ? r.description : null, struck: r.type === "stop" && r.struck, depth: r.depth })),
    });
  }, [shown, listView, buttons.map((b) => b.text).join("|"), emptyOnText, route, folded, mode, count, tourId, view?.title, foot, canStop, more]); // eslint-disable-line react-hooks/exhaustive-deps

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
                <>
                  <div role="group" aria-label="Which tours" data-part="views" className="flex flex-none items-center gap-1 px-2 py-1">
                    {buttons.map((b) => (
                      <button
                        key={b.id}
                        type="button"
                        data-view={b.id}
                        aria-pressed={b.pressed}
                        disabled={b.disabled}
                        title={b.tip ?? undefined}
                        className={cn("min-w-0 truncate rounded-sm border px-2 py-0.5 text-[11px]", b.pressed ? "border-[color:var(--ring)] bg-[color:var(--list-hover-bg)] text-foreground" : "border-border text-muted-foreground hover:text-foreground", b.disabled && "opacity-50")}
                        onClick={() => switchView(b.id)}
                        onKeyDown={(e) => {
                          const to = viewKey(e.key, listView, selection !== null);
                          if (!to) return;
                          e.preventDefault();
                          e.stopPropagation();
                          switchView(to);
                          e.currentTarget.parentElement?.querySelector<HTMLElement>(`[data-view=${to}]`)?.focus();
                        }}
                      >
                        {b.text}
                      </button>
                    ))}
                  </div>
                  {emptyOnText !== null ? (
                    <div data-part="empty-on" className="px-3 py-2 text-[12px] text-muted-foreground">
                      <p>{emptyOnText}</p>
                      <button type="button" className="mt-1.5 rounded-sm border border-border px-2 py-0.5 text-[11px] text-foreground hover:bg-toolbar-hover" onClick={() => switchView("all")}>
                        {showAll(rows.length)}
                      </button>
                    </div>
                  ) : (
                    (shown ?? []).map((row, i) => <TourListRow key={`${row.id}:${row.folder}`} row={row} domId={`tours-list-${i}`} open={tourId} focused={cursor === i} line2={lineTwo(row, listView)} stateWord={stateWordOf(row, listView)} onOpen={() => openTour(row)} />)
                  )}
                </>
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
