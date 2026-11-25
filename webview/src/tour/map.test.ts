import assert from "node:assert/strict";
import { test } from "node:test";

import type { Place, StepKind, StepView, TourStop, TourView } from "@vnoc/shared/tours.ts";
import { CARD_H, CARD_W, COL_W, ROW_H } from "../canvas/layout.ts";
import { boxOf, callsShape, mapOf } from "./map.ts";

const step = (id: string, kind: StepKind): StepView => ({ id, kind, depth: 0, parent: null, title: null, text: "t", lines: null });
const stop = (n: number, over: Partial<TourStop> = {}): TourStop => ({
  number: n, id: `s${n}`, depth: 0, planned: false, kind: "function", name: `f${n}`, path: "a.py", qualname: `f${n}`, key: `a.py::f${n}`, lines: [10 * n, 10 * n + 4], steps: 1, blocks: [], reading: [step(`s${n}`, "intro")], ...over,
});
const place = (at: string, over: Partial<Place> = {}): Place => ({ at, seen: [], back: [], moved: "", by: "reader", open: true, stop: at === "cover" || at === "end" ? null : at.split(".")[0], position: null, unseen: 0, first_unseen: null, ...over });

// s1 · s2 reached from s1 by a call at line 13 · s3 inside s1 (containment) · s4 reached from s2 by a call, explained at s2 · s5 not written, from s3
const stops = [
  stop(1, { steps: 3 }),
  stop(2, { depth: 1, from: "s1", line: 13 }),
  stop(3, { depth: 1, from: "s1", kind: "class", name: "Box" }),
  stop(4, { depth: 2, from: "s2", line: 22, same_as: "s2" }),
  stop(5, { depth: 2, from: "s3", planned: true, lines: undefined, reading: [step("s5", "planned")] }),
];
const view = (at: string, over: Partial<TourView> = {}, p: Partial<Place> = {}): TourView => ({
  id: "t-1", title: "T", detail: "normal", state: "writing", folder: "private", written: 4, planned: 1, steps: 6, minutes: 1, pinned: false, writing: "s5", quiet: false, of: 5, offers: null, problems: [],
  place: place(at, p), stops, ...over,
});

test("there is one card per stop, from the first second, in the route's order: a row each, the depth to the right", () => {
  const { cards } = mapOf(view("cover"));
  assert.deepEqual(cards.map((c) => [c.id, c.number, c.x / COL_W, c.y / ROW_H]), [["s1", 1, 0, 0], ["s2", 2, 1, 1], ["s3", 3, 1, 2], ["s4", 4, 2, 3], ["s5", 5, 2, 4]]);
  assert.deepEqual(boxOf(cards[1]), { x: COL_W, y: ROW_H, width: CARD_W, height: CARD_H });
});

test("the badge is the stop's mark, and the ring is on the stop the reader is on: from the view alone", () => {
  const { cards } = mapOf(view("s2", {}, { seen: ["s1"] }));
  assert.deepEqual(cards.map((c) => [c.id, c.mark, c.here]), [["s1", "seen", false], ["s2", "here", true], ["s3", "ahead", false], ["s4", "ahead", false], ["s5", "planned", false]]);
  assert.deepEqual(mapOf(view("cover")).cards.map((c) => c.here), [false, false, false, false, false], "at the cover nobody is here");
  assert.equal(mapOf(view("end")).cards.filter((c) => c.here).length, 0);
});

test("a stop explained before says `⇢ explained at N`, and a stop being written says writing…", () => {
  const cards = mapOf(view("cover")).cards;
  assert.deepEqual([cards[3].description, cards[3].explainedAt], ["⇢ 2", 2]);
  assert.deepEqual([cards[4].description, cards[4].explainedAt], ["writing…", null]);
  assert.equal(cards[0].description, null);
});

test("a card's +N is the core's tourCalls answer for its stop, 0 for a stop it does not name", () => {
  assert.deepEqual(mapOf(view("cover"), { s1: 2, s3: 1 }).cards.map((c) => c.off), [2, 0, 1, 0, 0]);
  assert.deepEqual(mapOf(view("cover")).cards.map((c) => c.off), [0, 0, 0, 0, 0], "nothing asked yet");
});

test("a stop reached by a call is a call card with its line; one reached by containment is not; the first has neither", () => {
  const cards = mapOf(view("cover")).cards;
  assert.deepEqual(cards.map((c) => [c.id, c.from, c.callLine]), [["s1", null, null], ["s2", "s1", 13], ["s3", "s1", null], ["s4", "s2", 22], ["s5", "s3", null]]);
});

test("a nested stop is joined to its parent: a dashed edge for a call, a solid one for containment", () => {
  const { edges } = mapOf(view("cover"));
  assert.deepEqual(edges.map((e) => [e.source, e.target, e.kind]), [["s1", "s2", "call"], ["s1", "s3", "node"], ["s2", "s4", "call"], ["s3", "s5", "node"]]);
});

test("the edges from the first stop to the current one are heavy, and no others", () => {
  const heavy = (at: string) => mapOf(view(at)).edges.filter((e) => e.onPath).map((e) => e.id);
  assert.deepEqual(heavy("s4"), ["s1->s2", "s2->s4"]);
  assert.deepEqual(heavy("s2"), ["s1->s2"]);
  assert.deepEqual(heavy("s1"), [], "the first stop is the start of the way");
  assert.deepEqual(heavy("cover"), []);
  assert.deepEqual(heavy("s5"), ["s1->s3", "s3->s5"]);
});

test("a planned stop has no steps count and is drawn planned; a written stop with more than one step says how many", () => {
  const cards = mapOf(view("cover")).cards;
  assert.deepEqual(cards.map((c) => c.steps), [3, null, null, null, null]);
  assert.deepEqual([cards[4].planned, cards[4].line, cards[4].mark], [true, null, "planned"]);
  assert.equal(cards[0].line, 10);
});

test("a chain of `from` that loops, or points at nothing, draws no edge it cannot and does not hang", () => {
  const loop = [stop(1, { from: "s2" }), stop(2, { from: "s1" }), stop(3, { from: "s9" })];
  const { edges } = mapOf(view("s2", { stops: loop }));
  assert.deepEqual(edges.map((e) => e.id), ["s2->s1", "s1->s2"], "an edge to a stop the tour does not have is not drawn");
});

test("a card says gone, changed or nothing in the footer's right, and a gone stop's card is struck", () => {
  const checked = (check: TourStop["check"]) => mapOf(view("cover", { writing: null, stops: [stop(1, { check })] })).cards[0];
  assert.deepEqual([checked("gone").description, checked("gone").struck], ["gone", true]);
  assert.deepEqual([checked("changed").description, checked("changed").struck], ["changed", false]);
  assert.deepEqual([checked("moved").description, checked("moved").struck], [null, false], "a block that moved: no mark");
});

test("the badge of the stop the agent is on turns, unless the agent has been quiet; then the card says `no news`", () => {
  assert.deepEqual(mapOf(view("cover")).cards.map((c) => c.writing), [false, false, false, false, true]);
  const quiet = mapOf(view("cover", { quiet: true })).cards;
  assert.deepEqual(quiet.map((c) => c.writing), [false, false, false, false, false], "no news: nothing turns");
  assert.equal(quiet[4].description, "no news");
  assert.equal(quiet[4].planned, true, "it is still not written");
});

test("the calls are asked again when the stops change, not when the reader moves", () => {
  const at = (where: string) => callsShape(view(where));
  assert.strictEqual(at("cover"), at("s2"), "a move is not a change of the stops");
  assert.notStrictEqual(callsShape(view("cover")), callsShape(view("cover", { stops: [...stops.slice(0, 4), stop(5, { depth: 2, from: "s3" })] })), "a stop written");
  assert.notStrictEqual(callsShape(view("cover")), callsShape(view("cover", { stops: stops.map((s, i) => (i === 1 ? { ...s, check: "changed" as const } : s)) })), "a check said changed");
  assert.notStrictEqual(callsShape(view("cover")), callsShape(view("cover", { stops: stops.map((s, i) => (i === 1 ? { ...s, key: "b.py::g" } : s)) })), "a stop found elsewhere");
});
