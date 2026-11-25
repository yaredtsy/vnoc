import assert from "node:assert/strict";
import { test } from "node:test";

import type { TourRow } from "@vnoc/shared/tours.ts";
import { rowMenu, tourRowContext } from "./menu.ts";

const row = (over: Partial<TourRow> = {}): TourRow => ({ id: "t-1", folder: "private", readable: true, state: "finished", pinned: false, title: "Retry", ...over }) as TourRow;

test("a finished private tour that is not pinned has Pin, Commit and Pin…, Write IDs, Rename… and Share", () => {
  assert.deepEqual(rowMenu(row()), { open: true, pin: true, commitAndPin: true, writeIds: true, rename: true, share: true, makePrivate: false });
});

test("a pinned tour has Write IDs, not Pin; a project tour has Make Private Again, not Share", () => {
  assert.deepEqual(rowMenu(row({ pinned: true })), { open: true, pin: false, commitAndPin: false, writeIds: true, rename: true, share: true, makePrivate: false });
  assert.deepEqual(rowMenu(row({ folder: "project" })), { open: true, pin: true, commitAndPin: true, writeIds: true, rename: true, share: false, makePrivate: true });
});

test("a tour being written can be renamed but not pinned, given IDs or moved; a stopped one can be moved; the private copy of a shared tour cannot", () => {
  for (const state of ["writing", "stopping"] as const) assert.deepEqual(rowMenu(row({ state })), { open: true, pin: false, commitAndPin: false, writeIds: false, rename: true, share: false, makePrivate: false }, state);
  assert.deepEqual(rowMenu(row({ state: "stopped" })), { open: true, pin: false, commitAndPin: false, writeIds: false, rename: true, share: true, makePrivate: false });
  assert.deepEqual(rowMenu(row({ private_copy: true })).share, false);
});

test("a tour that cannot be read has no entry but Delete (which every row has)", () => {
  assert.deepEqual(rowMenu(row({ readable: false, state: undefined })), { open: false, pin: false, commitAndPin: false, writeIds: false, rename: false, share: false, makePrivate: false });
});

test("the context says the section, the tour and one flag for each entry", () => {
  const said = JSON.parse(tourRowContext(row()));
  assert.deepEqual(said, { webviewSection: "tourRow", preventDefaultContextMenuItems: true, tour: "t-1", title: "Retry", canOpen: true, canPin: true, canCommitAndPin: true, canWriteIds: true, canRename: true, canShare: true, canMakePrivate: false });
  assert.deepEqual([JSON.parse(tourRowContext(row({ pinned: true }))).canPin, JSON.parse(tourRowContext(row({ pinned: true }))).canWriteIds], [false, true]);
  assert.equal(JSON.parse(tourRowContext(row({ folder: "project" }))).canMakePrivate, true);
});
