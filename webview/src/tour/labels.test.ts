import assert from "node:assert/strict";
import { test } from "node:test";

import type { Place, StepView, TourStop, TourView } from "@vnoc/shared/tours.ts";
import { routeRows } from "../sidebar/tours/route.ts";
import { describedAs, placeLabel, routeLabel, stepLabel } from "./labels.ts";

const step = (id: string, over: Partial<StepView> = {}): StepView => ({ id, kind: "block", depth: 1, parent: null, title: null, text: "t", lines: null, ...over });
const stop = (id: string, reading: StepView[], over: Partial<TourStop> = {}): TourStop => ({
  number: Number(id.slice(1)), id, depth: 0, planned: false, kind: "function", name: id, path: "a.py", qualname: id, key: `a.py::${id}`, steps: reading.length, blocks: [], reading, ...over,
});
const place = (at: string, over: Partial<Place> = {}): Place => ({ at, seen: [], back: [], moved: "", by: "reader", open: true, stop: at.split(".")[0], position: null, unseen: 0, first_unseen: null, ...over });
const view = (p: Place, stops: TourStop[], over: Partial<TourView> = {}): TourView => ({
  id: "t-1", title: "T", detail: "normal", state: "finished", folder: "private", written: stops.length, planned: 0, steps: 6, minutes: 1, pinned: false, writing: null, quiet: false, of: 10, offers: null, problems: [], place: p, stops, ...over,
});

const reserve = stop("s3", [step("s3", { kind: "intro", depth: 0 }), step("s3.b1", { title: "Refuse what is not there" }), step("s3.b1.b1", { kind: "detail", depth: 2, parent: "s3.b1", title: "The guard" })], { name: "reserve_stock" });

test("a step is read as the stop, its kind and name, and the step: stop 3 of 10, function reserve_stock, step 2 of 3. Refuse what is not there", () => {
  const v = view(place("s3.b1", { position: { stop: 3, of: 10, step: 2, steps: 3 } }), [reserve]);
  assert.equal(stepLabel(v, reserve, reserve.reading[1], null), "stop 3 of 10, function reserve_stock, step 2 of 3. Refuse what is not there");
});

test("a block nested in another says detail of its parent", () => {
  const v = view(place("s3.b1.b1", { position: { stop: 3, of: 10, step: 3, steps: 3 } }), [reserve]);
  assert.equal(stepLabel(v, reserve, reserve.reading[2], reserve.reading[1]), "stop 3 of 10, function reserve_stock, step 3 of 3. detail of Refuse what is not there. The guard");
});

test("a stop with one step has no step part, an intro has no title, and a tour with no plan has no `of`", () => {
  const one = stop("s1", [step("s1", { kind: "short", depth: 0 })], { name: "f", kind: "class" });
  const v = view(place("s1", { position: { stop: 1, of: 1, step: 1, steps: 1 } }), [one]);
  assert.equal(stepLabel(v, one, one.reading[0], null), "stop 1 of 1, class f");
  const writing = view(place("s1", { position: { stop: 1, of: 1, step: 1, steps: 1 } }), [one], { of: null, written: 6, state: "writing" });
  assert.equal(placeLabel(writing, one), "stop 1, 6 so far, class f");
});

test("a route row: stop 6, function retry_charge, not written, the agent is writing it, level 2", () => {
  const retry = stop("s6", [step("s6", { kind: "planned", depth: 0 })], { name: "retry_charge", planned: true, depth: 1 });
  const v = view(place("s1"), [stop("s1", [step("s1", { kind: "intro", depth: 0 })]), retry], { writing: "s6", state: "writing" });
  const rows = routeRows(v);
  assert.equal(routeLabel(rows[1]), "stop 6, function retry_charge, not written, the agent is writing it, level 2");
  assert.equal(routeLabel(rows[0]), "stop 1, function s1, you are here, level 1");
});

test("a step row of the route says detail of its parent, and the one you are on", () => {
  const v = view(place("s3.b1.b1"), [reserve]);
  const rows = routeRows(v);
  assert.deepEqual(rows.filter((r) => r.type === "step").map(routeLabel), ["what it is for", "Refuse what is not there", "detail of Refuse what is not there: The guard, the step you are on"]);
});

test("the one word of a row is said in full", () => {
  assert.deepEqual(["writing…", "no news", "gone", "changed", "⇢ 2", null].map(describedAs), ["the agent is writing it", "no news from the agent", "its code is gone", "changed since it was written", "explained at stop 2", null]);
});
