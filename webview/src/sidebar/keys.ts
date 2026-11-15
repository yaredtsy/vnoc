// The keys of the STRUCTURE view (docs/16-ui-redesign/06-sidebar.md §7), except type-ahead (part 2). `sidebarKey` says what a key means for the rows as they
// are; the page dispatches the answer. Pure, so each row of the table is a test.

import type { Intent } from "@vnoc/shared/session.ts";
import type { FlatRow } from "./flatten.ts";

/** What a key asks for: a change of the session, or the click rule of the row (07 §4; not an intent: the host knows if the canvas is open). */
export type SidebarAction = { type: "intent"; intent: Intent } | { type: "click"; id: string };

export interface KeyFacts {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

const intent = (i: Intent): SidebarAction => ({ type: "intent", intent: i });

/**
 * Whether a key puts the cursor in the search box (06 §7): `/`, or `Ctrl/Cmd+F`. The page asks before anything else, from anywhere in the view but the box
 * itself (in the box `/` is a character: `billing/` finds by path).
 */
export function isSearchKey(e: KeyFacts): boolean {
  if (e.altKey) return false;
  if (e.ctrlKey || e.metaKey) return !e.shiftKey && e.key.toLowerCase() === "f";
  return e.key === "/";
}

/**
 * `rows` in row order, `selected` the selected row's id (a node with no row, or none, counts as no selection), `page` how many rows a screen holds.
 * `Shift+F10` and the context-menu key are left to the browser, which raises the row's menu; a letter is type-ahead (`typeAhead.ts`, which has the state).
 */
export function sidebarKey(e: KeyFacts, rows: readonly FlatRow[], selected: string | null, page: number): SidebarAction | null {
  const at = selected === null ? -1 : rows.findIndex((r) => r.id === selected);
  const row = at >= 0 ? rows[at] : null;
  if (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey) return e.key === "ArrowLeft" ? intent({ type: "focusBack" }) : null;
  if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey) return e.key === "Enter" && row ? intent({ type: "focus", id: row.id }) : null;
  if (e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) return null;
  if (rows.length === 0) return null;
  const select = (index: number) => intent({ type: "select", id: rows[Math.max(0, Math.min(index, rows.length - 1))].id });
  switch (e.key) {
    case "ArrowDown":
      return select(at + 1);
    case "ArrowUp":
      return select(at <= 0 ? 0 : at - 1);
    case "Home":
      return select(0);
    case "End":
      return select(rows.length - 1);
    case "PageDown":
      return select(at + Math.max(1, page));
    case "PageUp":
      return select(at - Math.max(1, page));
    case "ArrowRight": // open it if it is closed, else go to its first child
      if (!row || !row.hasChildren) return null;
      if (!row.open) return intent({ type: "expand", ids: [row.id] });
      return rows[at + 1] && rows[at + 1].depth > row.depth ? select(at + 1) : null;
    case "ArrowLeft": // close it if it is open (the focused node stays open), else go to its parent
      if (!row) return null;
      if (row.open && !row.focusRoot) return intent({ type: "toggle", id: row.id });
      return row.parent !== null ? intent({ type: "select", id: row.parent }) : null;
    case " ":
      return row && row.hasChildren && !row.focusRoot ? intent({ type: "toggle", id: row.id }) : null;
    case "Enter":
      return row ? { type: "click", id: row.id } : null;
    default:
      return null;
  }
}
