import assert from "node:assert/strict";
import { test } from "node:test";

import type { Place, StepView, TourStop, TourView } from "@vnoc/shared/tours.ts";
import { canGoBack, crumbsOf, movedByAgent, narrowAgent } from "./bar.ts";

const intro = (id: string): StepView => ({ id, kind: "intro", depth: 0, parent: null, title: null, text: "t", lines: null });
const stop = (n: number, over: Partial<TourStop> = {}): TourStop => ({ number: n, id: `s${n}`, depth: 0, planned: false, kind: "function", name: `f${n}`, path: "a.py", qualname: `f${n}`, key: `a.py::f${n}`, steps: 1, blocks: [], reading: [intro(`s${n}`)], ...over });
const place = (at: string, over: Partial<Place> = {}): Place => ({ at, seen: [], back: [], moved: "", by: "reader", open: true, stop: at === "cover" || at === "end" ? null : at.split(".")[0], position: null, unseen: 0, first_unseen: null, ...over });
const view = (at: string, over: Partial<Place> = {}, stops = [stop(1), stop(2, { from: "s1", line: 4 }), stop(3, { from: "s2" }), stop(4, { from: "s1" })]): TourView => ({
  id: "t-1", title: "T", detail: "normal", state: "finished", folder: "private", written: 4, planned: 0, steps: 4, minutes: 1, pinned: false, writing: null, quiet: false, of: 4, offers: null, problems: [], place: place(at, over), stops,
});

test("the crumbs are the stops from the first down to the current one, by the stop each was reached from; the project is not one", () => {
  assert.deepEqual(crumbsOf(view("s3")).map((c) => c.id), ["s1", "s2", "s3"]);
  assert.deepEqual(crumbsOf(view("s4")).map((c) => c.label), ["f1", "f4"], "a stop reached from the first: two crumbs, not the route's order");
  assert.deepEqual(crumbsOf(view("s1.b2")).map((c) => c.id), ["s1"], "a step is in its stop");
  assert.deepEqual(crumbsOf(view("cover")), [], "no current stop at the cover");
  assert.deepEqual(crumbsOf(view("end")), []);
});

test("a chain that loops or points at a stop the tour has not got ends there", () => {
  const loop = [stop(1, { from: "s2" }), stop(2, { from: "s1" })];
  assert.equal(crumbsOf(view("s2", {}, loop)).length <= 3, true, "it does not hang");
  assert.deepEqual(crumbsOf(view("s2", {}, [stop(1), stop(2, { from: "s9" })])).map((c) => c.id), ["s2"]);
});

test("`moved by the agent` is a place that moved since the last, and says a command moved it", () => {
  const was = place("s1", { moved: "2026-11-25T10:00:00Z" });
  assert.equal(movedByAgent(was, place("s2", { by: "command", moved: "2026-11-25T10:00:05Z" })), true);
  assert.equal(movedByAgent(was, place("s2", { by: "reader", moved: "2026-11-25T10:00:05Z" })), false, "the reader moved it");
  assert.equal(movedByAgent(was, place("s1", { by: "command", moved: "2026-11-25T10:00:00Z" })), false, "the same place again (a re-read)");
  assert.equal(movedByAgent(was, place("s3", { by: "command", moved: "2026-11-25T10:00:00Z" })), true, "another step in the same second");
  assert.equal(movedByAgent(null, place("s2", { by: "command", moved: "2026-11-25T10:00:05Z" })), false, "the first place the bar sees");
  assert.equal(movedByAgent(was, place("s2", { by: "command", moved: "" })), false, "a place nobody has moved");
});

test("← is enabled when the core has a place to go back to", () => {
  assert.equal(canGoBack(view("s2", { back: ["s1"] })), true);
  assert.equal(canGoBack(view("s2")), false);
});

test("on a narrow bar the agent says its numbers alone: 6/10, 6 so far; the other words stay", () => {
  assert.deepEqual(["writing 6/10", "writing · 6 so far", "stopping", "stopped", "no news"].map(narrowAgent), ["6/10", "6 so far", "stopping", "stopped", "no news"]);
});
