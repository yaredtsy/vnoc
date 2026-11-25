// The right-click menu of a tour's row in the TOURS list (docs/26-tours/05-on-screen.md §3, §5 "Decided for TR22a"). One rule, here, which entries show; the entries themselves are
// VS Code's menu (`package.json` `webview/context`, section `tourRow`) and read what `tourRowContext` says. The pin's and the IDs' entries (TR22a), **Rename…**, **Share with the Project** / **Make Private
// Again** and **Delete** (TR22b); **Open**, **Start Again** and **Reveal Root in Main** (TR22c).

import type { TourRow } from "@vnoc/shared/tours.ts";

/**
 * Which entries a row's menu has. **Open**, **Start Again** and **Reveal Root in Main** for a tour that can be read (one that cannot has only **Delete**). **Pin** and **Commit and Pin…** while the tour is finished and not pinned; **Write IDs for Its Stops** when it is finished. **Rename…** for a tour that can be read.
 * **Share with the Project** for a private one and **Make Private Again** for a project one, when the core can move it (not while it is being written). **Delete** is on every row, one that cannot be
 * read too (a tour being written is refused by the core, in words), so it has no flag.
 */
export interface RowMenu {
  open: boolean;
  pin: boolean;
  commitAndPin: boolean;
  writeIds: boolean;
  rename: boolean;
  share: boolean;
  makePrivate: boolean;
}

export function rowMenu(row: Pick<TourRow, "readable" | "state" | "pinned" | "folder" | "private_copy">): RowMenu {
  const finished = row.readable && row.state === "finished";
  const unpinned = finished && row.pinned !== true;
  const movable = row.readable && row.state !== "writing" && row.state !== "stopping" && row.private_copy !== true;
  return { open: row.readable, pin: unpinned, commitAndPin: unpinned, writeIds: finished, rename: row.readable, share: movable && row.folder === "private", makePrivate: movable && row.folder === "project" };
}

/** What a row tells VS Code about itself for its menu: the section, the tour, and one flag for each entry (`when` clauses read them). */
export function tourRowContext(row: Pick<TourRow, "id" | "title" | "readable" | "state" | "pinned" | "folder" | "private_copy">): string {
  const menu = rowMenu(row);
  return JSON.stringify({ webviewSection: "tourRow", preventDefaultContextMenuItems: true, tour: row.id, title: row.title ?? null, canOpen: menu.open, canPin: menu.pin, canCommitAndPin: menu.commitAndPin, canWriteIds: menu.writeIds, canRename: menu.rename, canShare: menu.share, canMakePrivate: menu.makePrivate });
}
