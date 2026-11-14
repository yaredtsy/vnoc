import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useMemo, useRef, useState } from "react";

import { MAIN } from "@vnoc/shared/session.ts";
import { post, request } from "../host.ts";
import { useSession } from "../state/session.ts";
import { act, clickRow } from "./actions.ts";
import type { FlatRow } from "./flatten.ts";
import { sidebarKey } from "./keys.ts";
import { ROW_H, Row } from "./Row.tsx";
import { inView, scrollFor } from "./viewport.ts";

/**
 * The rows as a virtual list with the ARIA of a tree (06 §1): one tab stop, the list itself, which names the selected row as its active descendant, so the
 * focus does not die when a scrolled-away row leaves the page. `rows` is the whole list; only the ones on screen (and a few more) are in the page.
 * What a key means is `sidebarKey`; what a click means is `actions.ts`. Draws; decides nothing.
 */
export function Tree({ rows, selected, cursor }: { rows: FlatRow[]; selected: string | null; cursor: string | null }) {
  const scroller = useRef<HTMLDivElement>(null);
  const [focused, setFocused] = useState(false);
  const contexts = useSession((s) => s.order.length);
  const virtual = useVirtualizer({ count: rows.length, getScrollElement: () => scroller.current, estimateSize: () => ROW_H, overscan: 12 });
  const index = useMemo(() => (selected === null ? -1 : rows.findIndex((r) => r.id === selected)), [rows, selected]);

  // The selected row is always in view when the selection changes, whoever changed it (a card, a reveal, a key). Rows are one height, so the scroll is
  // set directly (`scrollFor`): the virtualizer's own `scrollToIndex` measures and retries on later frames, and a list asked twice in a row could stop short.
  // It also runs when the row first exists (a reveal whose rows arrive after the selection) and when the list first has a height (the view was hidden).
  const measured = (virtual.scrollRect?.height ?? 0) > 0;
  const hasRow = index >= 0;
  useEffect(() => {
    const el = scroller.current;
    if (!el || !hasRow || !measured) return;
    const top = scrollFor(index, el.scrollTop, el.clientHeight, ROW_H);
    if (top !== el.scrollTop) el.scrollTop = top;
    // only when the selection moves, the row appears or the list gets a height: opening another row must not pull the list back to the selected one
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected, hasRow, measured]);

  // What the page reports is read from the element, not from the virtualizer: the virtualizer learns of a scroll from the element's `scroll` event, which
  // the browser raises on a frame, and a view that is not being painted raises none, so its `scrollOffset` can be a frame (or for ever) behind the element.
  // `offset` is only here to draw this effect again when the virtualizer sees a scroll (the user's); this effect comes after the one that scrolls, so it
  // reads the position that one set.
  const offset = virtual.scrollOffset ?? 0;
  const height = virtual.scrollRect?.height ?? 0;
  const reported = useRef("");
  useEffect(() => {
    const el = scroller.current;
    const height = el?.clientHeight ?? 0;
    const visible = inView(index, el?.scrollTop ?? 0, height, ROW_H);
    const note = { type: "sidebarRendered" as const, rows: rows.length, selected: index >= 0 ? selected : null, inView: visible, contexts, height };
    const key = JSON.stringify(note);
    if (key === reported.current) return;
    reported.current = key;
    post(note);
  }, [rows.length, selected, index, offset, contexts, height]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    const page = Math.max(1, Math.floor((virtual.scrollRect?.height ?? 0) / ROW_H) - 1);
    const action = sidebarKey(e, rows, selected, page);
    if (!action) return;
    e.preventDefault();
    if (action.type === "click") clickRow(action.id);
    else act(action.intent);
  };

  return (
    <div
      ref={scroller}
      role="tree"
      aria-label="Structure"
      aria-multiselectable={false}
      aria-activedescendant={index >= 0 ? `row-${index}` : undefined}
      tabIndex={0}
      className="group/tree min-h-0 flex-1 overflow-y-auto outline-none"
      onKeyDown={onKeyDown}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
    >
      <div className="relative w-full" style={{ height: virtual.getTotalSize() }}>
        {virtual.getVirtualItems().map((item) => {
          const row = rows[item.index];
          return (
            <Row
              key={row.id}
              row={row}
              index={item.index}
              start={item.start}
              contextId={MAIN}
              selected={row.id === selected}
              cursor={row.id === cursor}
              treeFocused={focused}
              onClick={() => {
                scroller.current?.focus({ preventScroll: true });
                clickRow(row.id);
              }}
              onDoubleClick={() => (row.node.kind === "folder" ? act({ type: "toggle", id: row.id }) : void request("goToCode", { key: row.id }).catch(() => {}))}
              onToggle={() => act({ type: "toggle", id: row.id })}
              onContextMenu={() => act({ type: "select", id: row.id })}
            />
          );
        })}
      </div>
    </div>
  );
}
