import assert from "node:assert/strict";
import { test } from "node:test";

import type { TourRow } from "@vnoc/shared/tours.ts";
import { rowMarks, stripOf } from "./writing.ts";

const row = (id: string, over: Partial<TourRow> = {}): TourRow => ({ id, folder: "private", readable: true, state: "finished", quiet: false, state_word: "finished", ...over }) as TourRow;

test("a tour being written has a live dot, the core's word and a Stop; after Stop the core's words are shown and the Stop goes", () => {
  assert.deepEqual(rowMarks(row("a", { state: "writing", state_word: "writing" }), null), { dot: "pulse", word: "writing", stop: true, playing: false });
  assert.deepEqual(rowMarks(row("a", { state: "writing", quiet: true, state_word: "no news" }), null), { dot: "still", word: "no news", stop: true, playing: false });
  assert.deepEqual(rowMarks(row("a", { state: "stopping", state_word: "stopping" }), null), { dot: "still", word: "stopping", stop: false, playing: false });
  assert.deepEqual(rowMarks(row("a", { state: "stopped", state_word: "stopped" }), null), { dot: null, word: "stopped", stop: false, playing: false });
});

test("a finished tour has no mark but the one it is played with", () => {
  assert.deepEqual(rowMarks(row("a"), null), { dot: null, word: null, stop: false, playing: false });
  assert.equal(rowMarks(row("a"), "a").playing, true);
  assert.equal(rowMarks(row("a"), "b").playing, false);
  assert.equal(rowMarks(row("a", { private_copy: true }), "a").playing, false, "the private copy of a shared tour is not the open one");
  assert.equal(rowMarks(row("a", { readable: false, state: undefined }), "a").playing, false);
});

test("the strip says how many tours are being written, with the one's Stop, and the open tour in the list", () => {
  const writing = (id: string) => row(id, { state: "writing" });
  assert.equal(stripOf([row("a"), row("b")], null, "list"), null);
  assert.deepEqual(stripOf([writing("a"), row("b")], null, "list"), { writing: "● writing", stop: "a", playing: null });
  assert.deepEqual(stripOf([writing("a"), writing("b")], null, "list"), { writing: "● 2 writing", stop: null, playing: null });
  assert.deepEqual(stripOf([row("a")], { id: "a", title: "Retry charge" }, "list"), { writing: null, stop: null, playing: "▶ Retry charge" });
});

test("in the route the open tour's own header says it is being written; another tour being written still shows", () => {
  const writing = (id: string) => row(id, { state: "writing" });
  const open = { id: "a", title: "T" };
  assert.equal(stripOf([writing("a")], open, "route"), null);
  assert.deepEqual(stripOf([writing("a"), writing("b")], open, "route"), { writing: "● 2 writing", stop: null, playing: null });
  assert.deepEqual(stripOf([row("a"), writing("b")], open, "route"), { writing: "● writing", stop: "b", playing: null });
});
