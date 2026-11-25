import assert from "node:assert/strict";
import { test } from "node:test";

import { agentWords, beatOf, footWords, nextState, soFar, statusBarText, stoppable, toAnnounce } from "./tourWriting.ts";
import type { Place, TourRow, TourStop, TourView } from "./tours.ts";

// The words of a tour being written, and when `Next` is dim: from the view alone (docs/26-tours/05-on-screen.md §3).

const stop = (n: number, planned: boolean, steps = 1): TourStop => ({
  number: n, id: `s${n}`, depth: 0, planned, kind: planned ? null : "function", name: `f${n}`, path: "a.py", qualname: `f${n}`, key: `a.py::f${n}`, steps, blocks: [],
  reading: Array.from({ length: steps }, (_, i) => ({ id: i === 0 ? `s${n}` : `s${n}.b${i}`, kind: planned ? ("planned" as const) : i === 0 ? ("intro" as const) : ("block" as const), depth: i === 0 ? 0 : 1, parent: null, title: null, text: "", lines: null })),
});
const place = (at: string, stopNo: number | null, step = 1, steps = 1, of = 3): Place => ({
  at, seen: [], back: [], moved: "", by: "reader", open: true, stop: stopNo === null ? null : `s${stopNo}`,
  position: stopNo === null ? null : { stop: stopNo, of, step, steps }, unseen: 0, first_unseen: null,
});
/** a tour of `written` written stops and `planned` planned ones, being written with `state` */
const tour = (written: number, planned: number, state: TourView["state"], at: Place, over: Partial<TourView> = {}): TourView => ({
  id: "t-1", title: "T", detail: "normal", state, folder: "private", written, planned, steps: written, minutes: 1, pinned: false,
  writing: state === "writing" && planned > 0 ? `s${written + 1}` : null, quiet: false,
  // the core's `of`: written + planned, and null while the tour is being written with no plan
  of: (state === "writing" || state === "stopping") && planned === 0 ? null : written + planned, offers: null, problems: [], place: at,
  stops: Array.from({ length: written + planned }, (_, i) => stop(i + 1, i >= written)),
  ...over,
});
const row = (id: string, over: Partial<TourRow> = {}): TourRow => ({ id, folder: "private", readable: true, state: "writing", quiet: false, root: null, place: null, offers: null, words: "", ...over });

test("the beat is the state, and no news when a tour that is being written has been quiet", () => {
  assert.equal(beatOf({ state: "writing", quiet: false }), "writing");
  assert.equal(beatOf({ state: "writing", quiet: true }), "quiet");
  assert.equal(beatOf({ state: "stopping", quiet: true }), "quiet", "the agent may have died before it saw the halt");
  assert.equal(beatOf({ state: "stopping", quiet: false }), "stopping");
  assert.equal(beatOf({ state: "stopped", quiet: true }), "stopped", "a stopped tour is not waiting for anybody");
  assert.equal(beatOf({ state: "finished", quiet: true }), "finished");
});

test("the size line of a tour with no plan says the stops so far", () => {
  assert.equal(soFar({ written: 1 }), "1 stop so far");
  assert.equal(soFar({ written: 6 }), "6 stops so far");
});

test("the bar's words, per state: with a plan the stop the agent is on of the size; with no plan the count so far", () => {
  const planned = tour(5, 5, "writing", place("s2", 2, 1, 1, 10));
  assert.equal(agentWords(planned), "writing 6/10");
  assert.equal(agentWords({ ...planned, quiet: true }), "no news");
  assert.equal(agentWords(tour(0, 10, "writing", place("cover", null))), "writing 1/10", "nothing written yet: the agent is on stop 1");
  assert.equal(agentWords(tour(6, 0, "writing", place("s2", 2))), "writing · 6 so far", "no plan: no `of`");
  assert.equal(agentWords(tour(5, 5, "stopping", place("s2", 2))), "stopping");
  assert.equal(agentWords(tour(5, 5, "stopped", place("s2", 2))), "stopped");
  assert.equal(agentWords(tour(10, 0, "finished", place("s2", 2))), null, "nothing on the bar for a finished tour");
});

test("the ■ is there only while the agent writes and has not been told to stop", () => {
  assert.equal(stoppable({ state: "writing", quiet: false }), true);
  assert.equal(stoppable({ state: "writing", quiet: true }), false, "no news: nobody to hear it");
  assert.equal(stoppable({ state: "stopping", quiet: false }), false, "pressed already");
  assert.equal(stoppable({ state: "stopped", quiet: false }), false);
  assert.equal(stoppable({ state: "finished", quiet: false }), false);
});

test("the route's foot, per state", () => {
  assert.equal(footWords(tour(5, 5, "writing", place("s2", 2))), "writing 6 of 10");
  assert.equal(footWords(tour(6, 0, "writing", place("s2", 2))), "writing · 6 stops so far");
  assert.equal(footWords(tour(5, 5, "stopping", place("s2", 2))), "stopping: the agent finishes the stop it is on");
  assert.equal(footWords(tour(5, 5, "stopped", place("s2", 2))), "stopped: 5 of 10 written");
  assert.equal(footWords(tour(6, 0, "stopped", place("s2", 2))), "stopped: 6 stops written");
  assert.equal(footWords({ ...tour(5, 5, "writing", place("s2", 2)), quiet: true }), "no news from the agent · 5 of 10 written");
  assert.equal(footWords({ ...tour(6, 0, "writing", place("s2", 2)), quiet: true }), "no news from the agent · 6 stops so far");
  assert.equal(footWords(tour(10, 0, "finished", place("s2", 2))), null);
});

test("the status bar item says the place by the core's numbers, and the agent's words while it writes", () => {
  assert.equal(statusBarText(null), null, "no tour is open: no item");
  assert.equal(statusBarText(tour(10, 0, "finished", place("s3", 3, 1, 1, 10))), "▶ Tour 3/10");
  assert.equal(statusBarText(tour(5, 5, "writing", place("s3", 3, 1, 1, 10))), "▶ Tour 3/10 · writing 6/10");
  assert.equal(statusBarText(tour(5, 5, "writing", place("cover", null))), "▶ Tour · writing 6/10", "the cover has no place");
  assert.equal(statusBarText({ ...tour(5, 5, "writing", place("s3", 3, 1, 1, 10)), quiet: true }), "▶ Tour 3/10 · no news");
  assert.equal(statusBarText(tour(5, 5, "stopped", place("s3", 3, 1, 1, 10))), "▶ Tour 3/10 · stopped");
});

test("Next is dim before a stop that is not written, and before the stop that has not been added (no plan); never inside a stop or at the cover", () => {
  // stops 1 and 2 written, 3 planned: on the last step of 2, Next goes to a stop that is not written
  const planned = (at: Place) => tour(2, 1, "writing", at);
  assert.equal(nextState(planned(place("s2", 2, 1, 1))), "planned");
  assert.equal(nextState(planned(place("s1", 1, 1, 1))), "ready", "stop 2 is written");
  assert.equal(nextState(planned(place("s1", 1, 1, 3))), "ready", "a step inside a stop: the next one is in the same stop");
  assert.equal(nextState(planned(place("s2", 2, 3, 3))), "planned", "the last step of stop 2");
  assert.equal(nextState(planned(place("cover", null))), "ready", "Start is always ready");
  assert.equal(nextState(planned(place("end", null))), "ready");
  // a tour with no plan, being written: its last written step has nothing after it yet
  const noPlan = (at: Place, state: TourView["state"] = "writing") => tour(2, 0, state, at);
  assert.equal(nextState(noPlan(place("s2", 2, 1, 1, 2))), "adding");
  assert.equal(nextState(noPlan(place("s1", 1, 1, 1, 2))), "ready");
  assert.equal(nextState(noPlan(place("s2", 2, 1, 1, 2), "finished")), "ready", "a finished tour: Finish");
  assert.equal(nextState(noPlan(place("s2", 2, 1, 1, 2), "stopped")), "ready", "a stopped tour will not add more");
  assert.equal(nextState({ ...noPlan(place("s2", 2, 1, 1, 2)), quiet: true }), "ready", "no news: the end card is reachable");
  // the same stop, the planned stop is written: Next lights
  assert.equal(nextState(tour(3, 0, "writing", place("s2", 2, 1, 1, 3))), "ready");
});

test("a tour that appears while the window is open and is being written is announced once", () => {
  const first = [row("t-1", { state: "finished" })];
  assert.deepEqual(toAnnounce(first, null, new Set()), [], "no list was held: the window just started");
  const known = new Set(["t-1"]);
  const after = [row("t-2"), ...first];
  assert.deepEqual(toAnnounce(after, known, new Set()).map((r) => r.id), ["t-2"]);
  assert.deepEqual(toAnnounce(after, known, new Set(["t-2"])), [], "once per tour");
  assert.deepEqual(toAnnounce([row("t-3", { state: "finished" })], known, new Set()), [], "a tour that is not being written: a pull, or one that was finished at once");
  assert.deepEqual(toAnnounce([row("t-4", { readable: false })], known, new Set()), [], "one that cannot be read");
  assert.deepEqual(toAnnounce(first, known, new Set()), [], "a tour the window knew");
});
