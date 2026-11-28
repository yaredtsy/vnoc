import assert from "node:assert/strict";
import { test } from "node:test";

import type { AsWritten, StepView, TourStop, TourView } from "@vnoc/shared/tours.ts";
import { coverNotes, pinNote, wantsWritten, writtenLook } from "./written.ts";

// How a step the code no longer matches is shown, and what the cover says about the pin and the code that moved on (docs/26-tours/05-on-screen.md §3).

const step = (over: Partial<StepView> = {}): StepView => ({ id: "s1.b1", kind: "block", depth: 1, parent: null, title: "T", text: "t", lines: [6, 7], fresh: "changed", ...over });
const stop = (over: Partial<TourStop> = {}): TourStop => ({ number: 1, id: "s1", depth: 0, planned: false, kind: "function", name: "f", path: "a.py", qualname: "f", key: "a.py::f", steps: 2, blocks: [], reading: [], check: "changed", ...over });
const answer: AsWritten = { commit: "8a1c2e4", path: "a.py", lines: [4, 14], code: ["def f():"], lit: [[6, 7]], steps: { s1: [4, 14], "s1.b1": [6, 7] } };
const pinned = { pinned: true, pin_in_repository: true };
const look = (over: Partial<Parameters<typeof writtenLook>[0]> = {}) => writtenLook({ view: pinned, stop: stop(), step: step(), codeLook: "lit", answer, choice: "written", ...over });

test("a step the code still matches is shown as it is: nothing is added", () => {
  assert.deepEqual(look({ stop: stop({ check: undefined }), step: step({ fresh: "fresh" }) }), { look: "fresh" });
  assert.deepEqual(look({ stop: stop({ check: "moved" }), step: step({ fresh: "fresh" }) }), { look: "fresh" }, "a block that moved is fresh, at its new lines: no mark");
  assert.deepEqual(look({ stop: stop({ check: "changed" }), step: step({ fresh: "fresh" }) }), { look: "fresh" }, "another step of a changed stop: as now");
  assert.deepEqual(look({ codeLook: "rows" }), { look: "fresh" }, "a folder's or a file's contents have no code to mark");
  assert.deepEqual(look({ codeLook: "plain" }), { look: "fresh" });
});

test("pinned, the commit is here and the core answered: the step opens as written, with Now offered", () => {
  assert.deepEqual(look(), { look: "written", switch: true, gone: false });
  assert.deepEqual(look({ choice: "now" }), { look: "now" }, "the reader asked for the code as it is");
  assert.deepEqual(look({ answer: undefined }), { look: "asking" }, "until the answer comes");
});

test("pinned, but the commit is not in this repository, or the core had no code to give: the dashed look", () => {
  assert.deepEqual(look({ view: { pinned: true, pin_in_repository: false } }), { look: "dashed" });
  assert.deepEqual(look({ answer: null }), { look: "dashed" });
  assert.equal(wantsWritten({ pinned: true, pin_in_repository: undefined }, stop(), step(), "lit"), false, "git was not asked: nothing to ask the core for");
});

test("not pinned: the dashed look, and the core is not asked for the code as written", () => {
  assert.deepEqual(look({ view: { pinned: false, pin_in_repository: undefined } }), { look: "dashed" });
  assert.equal(wantsWritten({ pinned: false, pin_in_repository: undefined }, stop(), step(), "lit"), false);
  assert.equal(wantsWritten(pinned, stop(), step(), "lit"), true);
  assert.equal(wantsWritten(pinned, stop(), step({ fresh: "fresh" }), "lit"), false, "a step that did not change");
});

test("a stop that is gone: as written with no Now when it can be shown, else no code pane", () => {
  const gone = stop({ check: "gone", key: null });
  assert.deepEqual(look({ stop: gone }), { look: "written", switch: false, gone: true });
  assert.deepEqual(look({ stop: gone, choice: "now" }), { look: "written", switch: false, gone: true }, "there is no now");
  assert.deepEqual(look({ stop: gone, step: step({ fresh: "fresh" }) }), { look: "written", switch: false, gone: true }, "every step of a gone stop");
  assert.deepEqual(look({ stop: gone, view: { pinned: true, pin_in_repository: false } }), { look: "gone" });
  assert.deepEqual(look({ stop: gone, view: { pinned: false, pin_in_repository: undefined } }), { look: "gone" });
  assert.deepEqual(look({ stop: gone, answer: null }), { look: "gone" });
  assert.deepEqual(look({ stop: gone, answer: undefined }), { look: "asking" });
});

test("the cover says where the tour is pinned, or that it is not; nothing while it is being written", () => {
  const pin = "6d0e3dd6479c54f027dcd25032e2da38bd084663";
  assert.deepEqual(pinNote({ state: "finished", pinned: true, pin, pin_in_repository: true }), { text: "pinned at 6d0e3dd", title: pin });
  assert.deepEqual(pinNote({ state: "finished", pinned: true, pin, pin_in_repository: false }), { text: "pinned at 6d0e3dd, a commit this repository does not have", title: pin });
  assert.deepEqual(pinNote({ state: "finished", pinned: false }), { text: "not pinned" });
  assert.deepEqual(pinNote({ state: "stopped", pinned: false }), { text: "not pinned" });
  assert.equal(pinNote({ state: "writing", pinned: false }), null);
  assert.equal(pinNote({ state: "stopping", pinned: false }), null);
});

test("the cover says when the first stop is gone, and when most of the tour is about older code", () => {
  const check = (changed: number, gone: number) => ({ fresh: false, from: "check" as const, at: "", changed, gone });
  const stops = (first: TourStop["check"]) => [stop({ name: "checkout", check: first }), stop({ id: "s2", name: "g" })];
  assert.deepEqual(coverNotes({ stops: stops("gone"), check: check(0, 1), written: 4 }), { gone: "checkout is no longer in the code. The tour shows what was written about it.", most: null });
  assert.deepEqual(coverNotes({ stops: stops("changed"), check: check(2, 1), written: 4 }), { gone: null, most: "Most of this tour is about older code. Ask the agent for a new one." });
  assert.deepEqual(coverNotes({ stops: stops(undefined), check: check(2, 0), written: 4 }), { gone: null, most: null }, "half is not most");
  assert.deepEqual(coverNotes({ stops: stops(undefined), written: 4 }), { gone: null, most: null }, "no check was made");
});
