// What the STRUCTURE view shows instead of rows, or beside none (docs/16-ui-redesign/06-sidebar.md §8). Pure, so the five cases are tested.

export type Status = "loading" | "ready" | "error";
export type SidebarBody = "noFolder" | "loading" | "empty" | "error" | "ready";

/**
 * No folder open: say so, whatever else. Rows already drawn stay whatever a later read says (the canvas does the same). With none yet: the first read is on
 * its way, or why it failed; a read that found no node says there is no Python here. `structure`: what the last read brought, null before the first.
 */
export function sidebarBody(folder: boolean, status: Status, structure: { root: { children: readonly unknown[] } } | null): SidebarBody {
  if (!folder) return "noFolder";
  if (structure) return structure.root.children.length > 0 ? "ready" : "empty";
  return status === "error" ? "error" : "loading";
}
