import assert from "node:assert/strict";
import { test } from "node:test";

import type { Place, StepKind, StepView, TourStop, TourView } from "@vnoc/shared/tours.ts";
import { ADDING_LINE, becameWritten, callLines, currentOf, keyMove, lineLook, lookOf, nextLabel, notWrittenSentence, positionWords, segmentsOf } from "./layout.ts";

const step = (id: string, kind: StepKind, over: Partial<StepView> = {}): StepView => ({ id, kind, depth: kind === "block" ? 1 : kind === "detail" ? 2 : 0, parent: null, title: null, text: "t", lines: null, ...over });
const stop = (id: string, reading: StepView[], over: Partial<TourStop> = {}): TourStop => ({
  number: Number(id.slice(1)), id, depth: 0, planned: false, kind: "function", name: id, path: "a.py", qualname: id, key: `a.py::${id}`, steps: reading.length, blocks: [], reading, ...over,
});
const place = (at: string, over: Partial<Place> = {}): Place => ({ at, seen: [], back: [], moved: "", by: "reader", open: true, stop: at === "cover" || at === "end" ? null : at.split(".")[0], position: null, unseen: 0, first_unseen: null, ...over });
const view = (at: string, over: Partial<TourView> = {}): TourView => ({
  id: "t-1", title: "T", detail: "normal", state: "finished", folder: "private", written: 3, planned: 0, steps: 6, minutes: 1, pinned: false, writing: null, quiet: false, of: 3, offers: null, problems: [],
  place: place(at),
  stops: [
    stop("s1", [step("s1", "intro"), step("s1.b1", "block", { lines: [3, 6] }), step("s1.b2", "detail", { parent: "s1.b1", lines: [4, 4] })]),
    stop("s2", [step("s2", "short")]),
    stop("s3", [step("s3", "planned")], { planned: true, key: null }),
  ],
  ...over,
});

test("the place is found in the view: the stop and the step it names, a detail's parent, and none at the cover or the end", () => {
  const c = currentOf(view("s1.b2"));
  assert.deepEqual([c.mode, c.stop?.id, c.step?.id, c.parent?.id], ["step", "s1", "s1.b2", "s1.b1"]);
  assert.deepEqual([currentOf(view("s2")).step?.kind, currentOf(view("s2")).parent], ["short", null]);
  for (const at of ["cover", "end"] as const) assert.deepEqual([currentOf(view(at)).mode, currentOf(view(at)).stop, currentOf(view(at)).step], [at, null, null]);
  assert.equal(currentOf(view("s9", { place: place("s9") })).step, null, "a place the view does not have draws nothing");
});

// the stage's choice of layout per kind of step (design 03 §3)
const rows: [StepKind | "cover" | "end", string, boolean, boolean, boolean, boolean, boolean][] = [
  // kind, code, title, lead, parent line, explained, bars
  ["intro", "intro", false, true, false, false, false],
  ["block", "lit", true, false, false, false, false],
  ["detail", "detail", true, false, true, false, false],
  ["short", "all", false, true, false, false, false],
  ["contents", "rows", false, true, false, false, false],
  ["same_as", "dim", false, false, false, true, false],
  ["planned", "plain", false, false, false, false, true],
  ["cover", "none", false, false, false, false, false],
  ["end", "none", false, false, false, false, false],
];
for (const [kind, code, title, lead, parentLine, explained, bars] of rows) {
  test(`the layout of a ${kind}`, () => {
    const v = kind === "cover" || kind === "end" ? view(kind) : view("s1", { stops: [stop("s1", [step("s1", kind)])] });
    assert.deepEqual(lookOf(currentOf(v)), { code, title, lead, parentLine, explained, bars });
  });
}

test("a line of the code is lit, dim, marked thin or plain by the look of its step", () => {
  const look = (code: Parameters<typeof lineLook>[0], n: number, lit: [number, number] | null = [12, 13], parent: [number, number] | null = [11, 16]) => lineLook(code, n, 10, lit, parent);
  assert.deepEqual([10, 11, 12].map((n) => look("intro", n)), ["thin", "plain", "plain"], "an intro: a thin mark on the first line, nothing lit, nothing dim");
  assert.deepEqual([10, 12, 13, 14].map((n) => look("lit", n)), ["dim", "lit", "lit", "dim"]);
  assert.equal(look("lit", 12, null), "plain", "a block with no lines lights nothing");
  assert.deepEqual([10, 11, 12, 14, 17].map((n) => look("detail", n)), ["dim", "thin", "lit", "thin", "dim"], "a detail: its lines lit, its parent's keep the thin mark, the rest dim");
  assert.deepEqual([10, 99].map((n) => look("all", n)), ["lit", "lit"]);
  assert.deepEqual([10, 99].map((n) => look("dim", n)), ["dim", "dim"]);
  assert.deepEqual([10, 12].map((n) => look("plain", n)), ["plain", "plain"]);
});

test("the segments are one per stop: filled where the reader is or has been, dotted when not written", () => {
  const v = view("s2", { place: place("s2", { seen: ["s1"] }) });
  assert.deepEqual(segmentsOf(v).map((s) => [s.stop, s.filled, s.planned, s.current]), [["s1", true, false, false], ["s2", true, false, true], ["s3", false, true, false]]);
  assert.deepEqual(segmentsOf(view("cover")).map((s) => s.filled), [false, false, false]);
});

test("the primary button says Start on the cover, Finish on the last step, Next elsewhere; the position says the second part only for a stop with steps", () => {
  const at = (a: string, position: Place["position"]) => view(a, { place: place(a, { position }) });
  assert.equal(nextLabel(view("cover")), "Start");
  assert.equal(nextLabel(at("s1", { stop: 1, of: 3, step: 1, steps: 3 })), "Next");
  assert.equal(nextLabel(at("s3", { stop: 3, of: 3, step: 1, steps: 1 })), "Finish");
  assert.equal(nextLabel(at("s3", { stop: 3, of: 3, step: 1, steps: 2 })), "Next");
  assert.equal(positionWords(at("s1.b1", { stop: 1, of: 3, step: 2, steps: 3 })), "stop 1 of 3 · 2 of 3");
  assert.equal(positionWords(at("s2", { stop: 2, of: 3, step: 1, steps: 1 })), "stop 2 of 3");
  assert.equal(positionWords(view("cover")), "");
});

test("the keys: → and Space are Next, ← is Prev, Home the cover, End the end card, Ctrl or Cmd+Enter Go to Code; a modified arrow is not ours", () => {
  const none = { ctrl: false, meta: false, alt: false };
  assert.deepEqual(["ArrowRight", " ", "ArrowLeft", "Home", "End", "x", "Enter"].map((k) => keyMove(k, none)), ["next", "next", "prev", "cover", "end", null, null]);
  assert.equal(keyMove("Enter", { ...none, ctrl: true }), "goToCode");
  assert.equal(keyMove("Enter", { ...none, meta: true }), "goToCode");
  assert.equal(keyMove("ArrowLeft", { ...none, alt: true }), "history", "Alt+← is back to where you were");
  assert.equal(keyMove("Escape", none), "closeTour");
  assert.equal(keyMove("ArrowRight", { ...none, alt: true }), null);
  assert.equal(keyMove("ArrowRight", { ...none, meta: true }), null);
  assert.equal(keyMove("Escape", { ...none, ctrl: true }), null);
});

test("the calls of a stop's code that are stops of the tour are the stops reached from it by a call, by line; nothing is matched by name", () => {
  const stops = [
    stop("s1", [step("s1", "intro")]),
    stop("s2", [step("s2", "short")], { from: "s1", line: 6 }),
    stop("s3", [step("s3", "short")], { from: "s1", line: 6 }),
    stop("s4", [step("s4", "short")], { from: "s1", line: 9 }),
    stop("s5", [step("s5", "short")], { from: "s1" }),
    stop("s6", [step("s6", "short")], { from: "s2", line: 3 }),
  ];
  const calls = callLines(view("s1", { stops }), "s1");
  assert.deepEqual([...calls].map(([line, s]) => [line, s.id]), [[6, "s2"], [9, "s4"]], "the first stop on the route for a line; a stop reached by containment is not a call");
  assert.deepEqual([...callLines(view("s1", { stops }), "s2")].map(([line, s]) => [line, s.id]), [[3, "s6"]]);
  assert.equal(callLines(view("s1", { stops }), "s4").size, 0);
});

// ── a tour being written (docs/26-tours/05-on-screen.md §3) ──────────────────────────────────────────────────────────────

const writing = (at: string, over: Partial<TourView> = {}) => view(at, { state: "writing", written: 2, planned: 1, writing: "s3", ...over });

test("the position says `of` with a plan and the count so far without one; the segments get one dotted one after the stops of a tour with no plan", () => {
  const position = { stop: 2, of: 2, step: 1, steps: 1 };
  const noPlan = view("s2", { state: "writing", written: 2, planned: 0, of: null, writing: null, place: place("s2", { position }), stops: view("s1").stops.slice(0, 2) });
  assert.equal(positionWords(noPlan), "stop 2 · 2 so far", "no plan, being written: no `of`");
  assert.deepEqual(segmentsOf(noPlan).map((s) => [s.stop, s.planned, s.adding ?? false]), [["s1", false, false], ["s2", false, false], ["", true, true]], "the stop the agent has not added yet");
  const planned = writing("s2", { place: place("s2", { position: { stop: 2, of: 3, step: 1, steps: 1 } }) });
  assert.equal(positionWords(planned), "stop 2 of 3", "a plan: the size is known");
  assert.equal(segmentsOf(planned).length, 3, "a plan has its stops, and no extra segment");
  assert.equal(positionWords({ ...noPlan, state: "finished", of: 2 }), "stop 2 of 2", "finished: of");
  assert.equal(segmentsOf({ ...noPlan, state: "stopped", of: 2 }).length, 2, "a stopped tour does not grow");
  assert.equal(positionWords({ ...noPlan, place: place("s2", { position: { ...position, steps: 3, step: 2 } }) }), "stop 2 · 2 so far · 2 of 3");
});

test("Next says Next, not Finish, when it is dim: the last written step of a tour that is not finished is not the last step", () => {
  const position = { stop: 3, of: 3, step: 1, steps: 1 };
  const v = view("s3", { place: place("s3", { position }) });
  assert.equal(nextLabel(v), "Finish");
  assert.equal(nextLabel(v, "adding"), "Next");
  assert.equal(nextLabel(v, "planned"), "Next");
  assert.equal(nextLabel(view("cover"), "planned"), "Start", "Start is never dim");
});

test("what a step that is not written says: being written, where the agent is, stopping, stopped, no news", () => {
  const v = writing("s3", { writing: "s3" });
  const s3 = { id: "s3" };
  assert.equal(notWrittenSentence(v, s3, "writing"), "The agent is writing this stop. You can read the code meanwhile.");
  assert.equal(notWrittenSentence({ ...v, writing: "s2" }, s3, "writing"), "Not written yet: the agent is on stop 2.", "a stop further on says where the agent is");
  assert.equal(notWrittenSentence({ ...v, writing: null }, s3, "writing"), "Not written yet.");
  assert.equal(notWrittenSentence(v, s3, "stopped"), "Not written: the tour was stopped here. Ask the agent to continue it.");
  assert.equal(notWrittenSentence(v, s3, "quiet"), "Not written. The agent may have stopped: look at its terminal, or ask it to continue the tour.");
  assert.equal(notWrittenSentence(v, s3, "stopping"), "Not written yet: the agent stops after the stop it is on.");
  assert.equal(ADDING_LINE, "The agent is adding the next stop.");
});

test("the text fades in only where the grey bars were: the same step, planned before and written now", () => {
  assert.equal(becameWritten({ id: "s3", kind: "planned" }, { id: "s3", kind: "intro" }), true);
  assert.equal(becameWritten({ id: "s3", kind: "planned" }, { id: "s3", kind: "short" }), true);
  assert.equal(becameWritten({ id: "s3", kind: "planned" }, { id: "s3", kind: "planned" }), false, "still not written");
  assert.equal(becameWritten({ id: "s2", kind: "planned" }, { id: "s3", kind: "intro" }), false, "the reader moved to another step");
  assert.equal(becameWritten({ id: "s3", kind: "intro" }, { id: "s3", kind: "intro" }), false);
  assert.equal(becameWritten(null, { id: "s3", kind: "intro" }), false, "the first drawing");
  assert.equal(becameWritten({ id: null, kind: null }, { id: "s3", kind: "intro" }), false);
});
