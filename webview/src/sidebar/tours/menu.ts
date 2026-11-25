// The right-click menu of a tour's row in the TOURS list (docs/26-tours/05-on-screen.md §3, §5 "Decided for TR22a"). One rule, here, which entries show; the entries themselves are
// VS Code's menu (`package.json` `webview/context`, section `tourRow`) and read what `tourRowContext` says. TR22a has the pin's and the IDs' entries; the rest of 05 §3's list is TR22b.

import type { TourRow } from "@vnoc/shared/tours.ts";

/** Which entries a row's menu has: the tour is finished (a tour being written cannot be pinned, nor its stops given IDs). **Pin** and **Commit and Pin…** while it is not pinned; **Write IDs for Its Stops** always. */
export interface RowMenu {
  pin: boolean;
  commitAndPin: boolean;
  writeIds: boolean;
}

export function rowMenu(row: Pick<TourRow, "readable" | "state" | "pinned">): RowMenu {
  const finished = row.readable && row.state === "finished";
  const unpinned = finished && row.pinned !== true;
  return { pin: unpinned, commitAndPin: unpinned, writeIds: finished };
}

/** What a row tells VS Code about itself for its menu: the section, the tour, and one flag for each entry (`when` clauses read them). */
export function tourRowContext(row: Pick<TourRow, "id" | "title" | "readable" | "state" | "pinned">): string {
  const menu = rowMenu(row);
  return JSON.stringify({ webviewSection: "tourRow", preventDefaultContextMenuItems: true, tour: row.id, title: row.title ?? null, canPin: menu.pin, canCommitAndPin: menu.commitAndPin, canWriteIds: menu.writeIds });
}
