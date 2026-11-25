import assert from "node:assert/strict";
import { test } from "node:test";

import type { Place, StepKind, StepView, TourStop, TourView } from "@vnoc/shared/tours.ts";
import { headerPlace, markOf, rowKey, routeRows } from "./route.ts";

const step = (id: string, kind: StepKind, over: Partial<StepView> = {}): StepView => ({ id, kind, depth: kind === "block" ? 1 : kind === "detail" ? 2 : 0, parent: null, title: null, text: "t", lines: null, ...over });
const stop = (n: number, over: Partial<TourStop> = {}): TourStop => ({ number: n, id: `s${n}`, depth: 0, planned: false, kind: "function", name: `f${n}`, path: "a.py", qualname: `f${n}`, key: `a.py::f${n}`, steps: 1, blocks: [], reading: [step(`s${n}`, "intro")], ...over });
const place = (at: string, over: Partial<Place> = {}): Place => ({ at, seen: [], back: [], moved: "", by: "reader", open: true, stop: at === "cover" || at === "end" ? null : at.split(".")[0], position: null, unseen: 0, first_unseen: null, ...over });

// s1 (three steps: a block and a detail) · s2 reached from s1 · s3 reached from s2 · s4 explained at s2 · s5 not written
const stops = [
  stop(1, { reading: [step("s1", "intro"), step("s1.b1", "block", { title: "Refuse what is not there" }), step("s1.b2", "detail", { title: "Look first", parent: "s1.b1" })] }),
  stop(2, { depth: 1, kind: "class", name: "Box" }),
  stop(3, { depth: 2, check: "changed" }),
  stop(4, { depth: 1, same_as: "s2", reading: [step("s4", "same_as")] }),
  stop(5, { planned: true, reading: [step("s5", "planned")] }),
];
const view = (at: string, over: Partial<TourView> = {}, p: Partial<Place> = {}): TourView => ({
  id: "t-1", title: "T", detail: "normal", state: "writing", folder: "private", written: 4, planned: 1, steps: 6, minutes: 1, pinned: false, writing: "s5", quiet: false, of: 5, offers: null, problems: [],
  place: place(at, p), stops, ...over,
});

test("the stops are in the view's order, nested by depth, each with its number, kind and name", () => {
  const rows = routeRows(view("cover")).filter((r) => r.type === "stop");
  assert.deepEqual(rows.map((r) => r.type === "stop" && [r.number, r.depth, r.kind, r.name]), [[1, 0, "function", "f1"], [2, 1, "class", "Box"], [3, 2, "function", "f3"], [4, 1, "function", "f4"], [5, 0, "function", "f5"]]);
});

test("the mark of a stop is read from the view: here, seen, ahead, or not written", () => {
  const v = view("s2", {}, { seen: ["s1"] });
  assert.deepEqual(routeRows(v).filter((r) => r.type === "stop").map((r) => r.type === "stop" && [r.id, r.mark, r.current]), [["s1", "seen", false], ["s2", "here", true], ["s3", "ahead", false], ["s4", "ahead", false], ["s5", "planned", false]]);
  assert.equal(markOf(view("s5"), "s5", true), "planned", "a stop that is not written is `◌` even when the reader is on it");
  assert.equal(markOf(view("s5", {}, { seen: ["s1"] }), "s1", false), "seen");
  assert.equal(markOf(view("cover"), "s1", false), "ahead", "at the cover nothing is here");
});

test("the steps of the stop you are in are under it, indented by their depth, the current one marked; no other stop shows its steps", () => {
  const rows = routeRows(view("s1.b1"));
  const at = rows.findIndex((r) => r.id === "s1");
  assert.deepEqual(rows.slice(at, at + 4).map((r) => [r.type, r.id, r.depth, r.type === "step" ? r.text : null, r.current]), [
    ["stop", "s1", 0, null, true],
    ["step", "s1", 0, "what it is for", false],
    ["step", "s1.b1", 1, "Refuse what is not there", true],
    ["step", "s1.b2", 2, "Look first", false],
  ]);
  assert.equal(rows.filter((r) => r.type === "step").length, 3);
  assert.equal(routeRows(view("s2")).filter((r) => r.type === "step").length, 0, "a stop with one step has no step rows");
  assert.equal(routeRows(view("cover")).filter((r) => r.type === "step").length, 0, "at the cover no stop is the one you are in");
});

test("a step under a nested stop is indented under that stop", () => {
  const deep = stop(3, { depth: 2, reading: [step("s3", "intro"), step("s3.b1", "block", { title: "B" })] });
  const rows = routeRows(view("s3", { stops: [stops[0], stops[1], deep] }));
  assert.deepEqual(rows.filter((r) => r.type === "step").map((r) => r.depth), [2, 3]);
});

test("a stop's one word: writing…, changed, or the stop it is explained at; the first that applies", () => {
  const words = (v: TourView) => routeRows(v).filter((r) => r.type === "stop").map((r) => r.type === "stop" && r.description);
  assert.deepEqual(words(view("cover")), [null, null, "changed", "⇢ 2", "writing…"]);
  assert.deepEqual(words(view("cover", { writing: null })), [null, null, "changed", "⇢ 2", null]);
  assert.deepEqual(words(view("cover", { writing: "s3" }))[2], "writing…", "writing comes before changed");
});

test("a stop being written says `no news` when the agent has been quiet", () => {
  const words = (v: TourView) => routeRows(v).filter((r) => r.type === "stop").map((r) => r.type === "stop" && r.description);
  assert.deepEqual(words(view("cover", { quiet: true })), [null, null, "changed", "⇢ 2", "no news"]);
  assert.deepEqual(words(view("cover", { quiet: false })), [null, null, "changed", "⇢ 2", "writing…"]);
});

test("the header's place is the core's numbers, and nothing at the cover and the end", () => {
  assert.equal(headerPlace(view("s2", {}, { position: { stop: 2, of: 5, step: 1, steps: 1 } })), "2 of 5");
  assert.equal(headerPlace(view("cover")), "");
});

test("a tour with no plan that is being written has no `of` in the header", () => {
  const position = { stop: 2, of: 4, step: 1, steps: 1 };
  assert.equal(headerPlace(view("s2", { planned: 0, of: null, writing: null }, { position })), "2 · 4 so far");
  assert.equal(headerPlace(view("s2", { planned: 0, of: 4, writing: null, state: "finished" }, { position })), "2 of 4");
});

test("every row of the route has a key of its own: a stop and its intro step have the same id", () => {
  const rows = routeRows(view("s1.b1"));
  assert.ok(rows.some((r) => r.type === "stop" && r.id === "s1") && rows.some((r) => r.type === "step" && r.id === "s1"), "both are in the route");
  const keys = rows.map(rowKey);
  assert.equal(new Set(keys).size, keys.length, keys.join(" "));
});
