import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useId, useMemo, useRef, useState } from "react";

import type { ContextId } from "@vnoc/shared/session.ts";
import { post, request } from "../host.ts";
import { dispatch } from "../state/session.ts";
import { toursOf, useTourCounts } from "../state/tourCounts.ts";
import { act, clickRow } from "./actions.ts";
import type { FlatRow } from "./flatten.ts";
import { isSearchKey, sidebarKey } from "./keys.ts";
import { ROW_H, Row } from "./Row.tsx";
import { toursChipTip } from "./tours/onView.ts";
import { type TypeAhead, noTypeAhead, typeAhead } from "./typeAhead.ts";
import { inView, scrollFor } from "./viewport.ts";

/** The pointer must rest on a row this long before the card is pointed at: crossing the list is not pointing at each card on the way. */
const HOVER_MS = 40;

/**
 * The rows as a virtual list with the ARIA of a tree (06 §1): one tab stop, the list itself, which names the selected row as its active descendant, so the
 * focus does not die when a scrolled-away row leaves the page. `rows` is the whole list; only the ones on screen (and a few more) are in the page.
 * What a key means is `sidebarKey` (and `typeAhead`); what a click means is `actions.ts`, in this panel's `context`. `onList` gets the list's element (the page
 * gives it the keyboard), `onTab` says where `Tab` goes: true when it went to another panel's list. Draws; decides nothing.
 */
export function Tree({ context, rows, selected, cursor, highlight, onList, onTab }: { context: ContextId; rows: FlatRow[]; selected: string | null; cursor: string | null; highlight: string | null; onList: (el: HTMLDivElement | null) => void; onTab: (shift: boolean) => boolean }) {
  const scroller = useRef<HTMLDivElement | null>(null);
  // a row's element id is unique across the panels' lists (the list names the selected one as its active descendant)
  const uid = useId();
  const rowDomId = (i: number) => `${uid}-row-${i}`;
  const [focused, setFocused] = useState(false);
  const virtual = useVirtualizer({ count: rows.length, getScrollElement: () => scroller.current, estimateSize: () => ROW_H, overscan: 12 });
  const index = useMemo(() => (selected === null ? -1 : rows.findIndex((r) => r.id === selected)), [rows, selected]);
  const typed = useRef<TypeAhead>(noTypeAhead);

  // The selected row is always in view when the selection changes, whoever changed it (a card, a reveal, a key). Rows are one height, so the scroll is
  // set directly (`scrollFor`): the virtualizer's own `scrollToIndex` measures and retries on later frames, and a list asked twice in a row could stop short.
  // It also runs when the row first exists (a reveal whose rows arrive after the selection) and when the list first has a height (the view was hidden).
  const measured = (virtual.scrollRect?.height ?? 0) > 0;
  const hasRow = index >= 0;
  useEffect(() => {
    const el = scroller.current;
    if (!el || !hasRow || !measured) return;
    const top = scrollFor(index, el.scrollTop, el.clientHeight, ROW_H);
    if (top !== el.scrollTop) {
      el.scrollTop = top;
      // The virtualizer reads the offset when the element raises `scroll`, which the browser does on a painted frame: in a view that is not being painted
      // (hidden, covered) it never comes, and the row would stay out of the page. Say it now; the browser's own event, when it comes, finds nothing to change.
      el.dispatchEvent(new Event("scroll"));
    }
    // only when the selection moves, the row appears or the list gets a height: opening another row must not pull the list back to the selected one
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected, hasRow, measured]);

  // What the page reports is read from the element, not from the virtualizer: the virtualizer learns of a scroll from the element's `scroll` event, which
  // the browser raises on a frame, and a view that is not being painted raises none, so its `scrollOffset` can be a frame (or for ever) behind the element.
  // `inView` is where the element is scrolled to; `drawn` is whether the virtualizer has put the selected row in the page: the two are different facts.
  // `offset` and `drawnRange` are only here to draw this effect again when the virtualizer sees a scroll (the user's) or changes what it draws; this effect
  // comes after the one that scrolls, so it reads the position that one set.
  const offset = virtual.scrollOffset ?? 0;
  const height = virtual.scrollRect?.height ?? 0;
  const items = virtual.getVirtualItems();
  const drawnRange = items.length > 0 ? `${items[0].index}-${items[items.length - 1].index}` : "";
  const reported = useRef("");
  useEffect(() => {
    const el = scroller.current;
    const note = {
      type: "sidebarRendered" as const,
      rows: rows.length,
      selected: index >= 0 ? selected : null,
      inView: inView(index, el?.scrollTop ?? 0, el?.clientHeight ?? 0, ROW_H),
      drawn: index >= 0 && document.getElementById(rowDomId(index)) !== null,
      context,
      height: el?.clientHeight ?? 0,
    };
    const key = JSON.stringify(note);
    if (key === reported.current) return;
    reported.current = key;
    post(note);
  }, [rows.length, selected, index, offset, drawnRange, context, height]);

  // the rows that are drawn and show `▶ N`: what the extension's test suite reads
  const counts = useTourCounts((s) => s.counts);
  const chipsReported = useRef("");
  useEffect(() => {
    const chips = items.flatMap((item) => {
      const n = toursOf(counts, rows[item.index]?.id ?? null);
      return n > 0 ? [{ id: rows[item.index].id, tours: n, tip: toursChipTip(n) }] : [];
    });
    const text = JSON.stringify({ context, chips });
    if (text === chipsReported.current) return;
    chipsReported.current = text;
    post({ type: "tourChips", surface: "sidebar", chips });
  }, [counts, rows, drawnRange, context]); // eslint-disable-line react-hooks/exhaustive-deps

  // The card of the row under the pointer is pointed at on the canvas (06 §6): the session's `highlight`, which every surface reads. Told after a short rest.
  const over = useRef<string | null>(null);
  const told = useRef<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const hover = (id: string | null) => {
    over.current = id;
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      if (over.current === told.current) return;
      told.current = over.current;
      dispatch({ type: "highlight", id: over.current });
    }, HOVER_MS);
  };
  useEffect(
    () => () => {
      clearTimeout(timer.current);
      if (told.current !== null) dispatch({ type: "highlight", id: null });
    },
    [],
  );

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (isSearchKey(e)) return; // the page puts the cursor in the search box
    if (e.key === "Tab" && !e.ctrlKey && !e.metaKey && !e.altKey && onTab(e.shiftKey)) return e.preventDefault(); // to the next panel's list (06 §7)
    const page = Math.max(1, Math.floor((virtual.scrollRect?.height ?? 0) / ROW_H) - 1);
    const action = sidebarKey(e, rows, selected, page);
    if (action) {
      e.preventDefault();
      if (action.type === "click") clickRow(context, action.id);
      else act(context, action.intent);
      return;
    }
    const found = typeAhead(typed.current, e, Date.now(), rows, selected);
    typed.current = found.state;
    if (found.id !== null) {
      e.preventDefault();
      act(context, { type: "select", id: found.id });
    }
  };

  return (
    <div
      ref={(el) => {
        scroller.current = el;
        onList(el);
      }}
      role="tree"
      aria-label="Structure"
      aria-multiselectable={false}
      aria-activedescendant={index >= 0 ? rowDomId(index) : undefined}
      tabIndex={0}
      className="group/tree min-h-0 flex-1 overflow-y-auto outline-none"
      onKeyDown={onKeyDown}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
      onMouseLeave={() => hover(null)}
    >
      <div className="relative w-full" style={{ height: virtual.getTotalSize() }}>
        {items.map((item) => {
          const row = rows[item.index];
          return (
            <Row
              key={row.id}
              row={row}
              domId={rowDomId(item.index)}
              start={item.start}
              contextId={context}
              selected={row.id === selected}
              highlighted={row.id === highlight}
              cursor={row.id === cursor}
              treeFocused={focused}
              onClick={() => {
                scroller.current?.focus({ preventScroll: true });
                clickRow(context, row.id);
              }}
              onDoubleClick={() => (row.node.kind === "folder" ? act(context, { type: "toggle", id: row.id }) : void request("goToCode", { key: row.id }).catch(() => {}))}
              onToggle={() => act(context, { type: "toggle", id: row.id })}
              onContextMenu={() => act(context, { type: "select", id: row.id })}
              onHover={(on) => hover(on ? row.id : null)}
            />
          );
        })}
      </div>
    </div>
  );
}
