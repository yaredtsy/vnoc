import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useMemo, useRef, useState } from "react";

import { MAIN } from "@vnoc/shared/session.ts";
import { post, request } from "../host.ts";
import { useSession } from "../state/session.ts";
import { act, clickRow } from "./actions.ts";
import type { FlatRow } from "./flatten.ts";
import { sidebarKey } from "./keys.ts";
import { ROW_H, Row } from "./Row.tsx";
import { inView } from "./viewport.ts";

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

  // the selected row is always in view when the selection changes, whoever changed it (a card, a reveal, a key)
  useEffect(() => {
    if (index >= 0) virtual.scrollToIndex(index, { align: "auto" });
    // only when the selection moves: opening another row must not pull the list back to the selected one
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected]);

  const visible = inView(index, virtual.scrollOffset ?? 0, virtual.scrollRect?.height ?? 0, ROW_H);
  useEffect(() => {
    post({ type: "sidebarRendered", rows: rows.length, selected: index >= 0 ? selected : null, inView: visible, contexts });
  }, [rows.length, selected, index, visible, contexts]);

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
