import assert from "node:assert/strict";
import { test } from "node:test";

import type { TourRow } from "@vnoc/shared/tours.ts";
import { rowMenu, tourRowContext } from "./menu.ts";

const row = (over: Partial<TourRow> = {}): TourRow => ({ id: "t-1", folder: "private", readable: true, state: "finished", pinned: false, title: "Retry", ...over }) as TourRow;

test("a finished tour that is not pinned has Pin, Commit and Pin… and Write IDs", () => {
  assert.deepEqual(rowMenu(row()), { pin: true, commitAndPin: true, writeIds: true });
});

test("a pinned tour has Write IDs only", () => {
  assert.deepEqual(rowMenu(row({ pinned: true })), { pin: false, commitAndPin: false, writeIds: true });
});

test("a tour being written, stopped, or unreadable has none of them", () => {
  for (const state of ["writing", "stopping", "stopped"] as const) assert.deepEqual(rowMenu(row({ state })), { pin: false, commitAndPin: false, writeIds: false }, state);
  assert.deepEqual(rowMenu(row({ readable: false, state: undefined })), { pin: false, commitAndPin: false, writeIds: false });
});

test("the context says the section, the tour and one flag for each entry", () => {
  const said = JSON.parse(tourRowContext(row()));
  assert.deepEqual(said, { webviewSection: "tourRow", preventDefaultContextMenuItems: true, tour: "t-1", title: "Retry", canPin: true, canCommitAndPin: true, canWriteIds: true });
  assert.deepEqual([JSON.parse(tourRowContext(row({ pinned: true }))).canPin, JSON.parse(tourRowContext(row({ pinned: true }))).canWriteIds], [false, true]);
});
