// The rules of lib/timing.mjs as tables (docs/18-release-0.2/04-gifs-v2.md §6.3). Run: `npm test` in tools/media.
import assert from "node:assert/strict";
import { test } from "node:test";

import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { readEdits } from "../lib/edit.mjs";
import { retime } from "../lib/timing.mjs";

const sum = (a) => a.reduce((x, y) => x + y, 0);
const near = (actual, want, what) => assert.ok(Math.abs(actual - want) < 0.011, `${what}: ${actual.toFixed(3)} is not ${want}`);

/** a film of one frame every 0.1 s for `seconds` seconds, with the marks, beats and freezes given */
const film = ({ seconds = 10, marks = [], beats = [{ name: "a", t: 0 }], freezes = [], times } = {}) => ({
  times: times ?? Array.from({ length: Math.round(seconds * 10) }, (_, i) => i / 10),
  end: seconds,
  marks,
  beats,
  freezes,
});
const mark = (type, from, to, extra = {}) => ({ type, label: "", from, to, ...extra });

test("nothing marked, nothing changed: every frame stays as long as it was", () => {
  const r = retime(film());
  near(sum(r.durations), 10, "total");
  near(sum(r.auto), 10, "auto");
  assert.equal(r.lines.length, 1);
});

test("the automatic pass: what each kind of mark becomes", () => {
  const table = [
    ["a wait of 3 s is cut to 0.3 s", [mark("wait", 1, 4)], 7.3],
    ["a wait of 0.2 s is left", [mark("wait", 1, 1.2)], 10],
    ["a hold after an action is trimmed to 0.6 s", [mark("hold", 2, 5, { kind: "after" })], 7.6],
    ["a hold with no kind is an `after`", [mark("hold", 2, 5)], 7.6],
    ["a hold to read is trimmed to 1.0 s", [mark("hold", 2, 5, { kind: "read" })], 8],
    ["the end hold is trimmed to 3.0 s", [mark("hold", 2, 9, { kind: "end" })], 6],
    ["the first hold is not trimmed", [mark("hold", 0, 3, { kind: "first" })], 10],
    ["a hold shorter than its kind is left", [mark("hold", 2, 2.4, { kind: "after" })], 10],
    ["typing of 3 s plays at 2×", [mark("typing", 2, 5)], 8.5],
    ["typing of 1.5 s is left", [mark("typing", 2, 3.5)], 10],
    ["glides, drags and reveals are not touched any more", [mark("glide", 1, 2), mark("drag", 3, 4), mark("reveal", 5, 6)], 10],
    ["marks together", [mark("wait", 0, 2), mark("hold", 2, 4, { kind: "after" }), mark("typing", 5, 8)], 10 - 1.7 - 1.4 - 1.5],
  ];
  for (const [what, marks, total] of table) near(sum(retime(film({ marks })).durations), total, what);
});

test("a wait keeps its end, the loaded state, and its beginning goes", () => {
  const r = retime(film({ marks: [mark("wait", 1, 4)] }));
  const at = (t) => r.durations[Math.round(t * 10)];
  near(at(1.5), 0, "inside the cut part");
  near(at(3.8), 0.1, "in the last 0.3 s");
});

test("a stretch nothing explains: capped at 0.7 s, unless a mark explains it or it is the end", () => {
  const sparse = [0, 0.3, 0.5, 3.5, 4, 4.5]; // the screen sent nothing from 0.5 to 3.5
  near(sum(retime(film({ seconds: 5, times: sparse })).durations), 5 - 2.3, "a frame that lasts 3 s is a freeze");
  near(sum(retime(film({ seconds: 5, times: sparse, marks: [mark("hold", 0.5, 3.5, { kind: "after" })] })).durations), 5 - 2.4, "inside a hold it is the hold's rule, not a freeze");
  near(sum(retime(film({ seconds: 5, times: [0, 0.5] })).durations), 5, "a last frame that lasts to the end is not cut");
  const found = retime(film({ seconds: 5, times: [0, 0.5, 1, 1.1, 1.2, 4, 4.5], freezes: [{ start: 1.2, end: 4 }] }));
  near(sum(found.durations), 5 - 2.1, "a freeze ffmpeg found");
  assert.ok(found.lines.some((l) => l.includes("not marked")), "and it is reported");
  assert.equal(retime(film({ marks: [mark("wait", 0, 0.2)] })).lines.length, 1, "nothing to report when nothing was changed");
});

test("the edit list: cut, speed, whole beat, hold", () => {
  const beats = [{ name: "a", t: 0 }, { name: "b", t: 5 }];
  const run = (edits, extra = {}) => retime(film({ beats, ...extra }), edits);
  const table = [
    ["cut a stretch of a beat", [{ beat: "b", from: 1, to: 2, cut: true }], 9],
    ["speed 2× on a stretch", [{ beat: "b", from: 1, to: 2, speed: 2 }], 9.5],
    ["speed 2× on a whole beat", [{ beat: "a", speed: 2 }], 7.5],
    ["speed from a time to the end of the beat", [{ beat: "a", from: 3, speed: 2 }], 9],
    ["cut and speed in one beat", [{ beat: "b", from: 0, to: 1, cut: true }, { beat: "b", from: 2, to: 4, speed: 4 }], 10 - 1 - 1.5],
    ["a speed under 1 slows it", [{ beat: "a", from: 0, to: 1, speed: 0.5 }], 11],
    ["no entries", [], 10],
  ];
  for (const [what, edits, total] of table) near(sum(run(edits).durations), total, what);
  near(sum(run([{ beat: "b", from: 1, to: 2, cut: true }]).auto), 10, "the automatic film is not changed by the list");
});

test("the edit list is on the clock of the review film: seconds since the beat began, after the automatic pass", () => {
  const marks = [mark("wait", 0, 3)]; // 2.7 s of the film go
  const beats = [{ name: "a", t: 0 }, { name: "b", t: 5 }];
  const r = retime(film({ marks, beats }), [{ beat: "b", from: 0, to: 0.5, cut: true }]);
  near(r.beats.find((b) => b.name === "b").start, 2.3, "b begins at 2.3 s of the review film");
  near(sum(r.durations), 10 - 2.7 - 0.5, "the cut is in b's first half second");
  const first = r.durations.slice(50, 55); // recorded 5.0 – 5.5, b's first half second
  near(sum(first), 0, "…and it is those frames");
  const kept = retime(film({ marks, beats })).durations.slice(50, 55);
  near(sum(kept), 0.5, "(they were there before)");
});

test("hold: the beat's last hold is set to the time given, shorter or longer", () => {
  const marks = [mark("hold", 2, 5, { kind: "read" })]; // 3 s, 1.0 s after the automatic pass
  const beats = [{ name: "a", t: 0 }, { name: "b", t: 6 }];
  const run = (hold) => retime(film({ marks, beats }), [{ beat: "a", hold }]);
  near(sum(retime(film({ marks, beats })).durations), 8, "automatic");
  near(sum(run(0.4).durations), 7.4, "shorter");
  near(sum(run(2.5).durations), 9.5, "longer: the frame on screen stays");
  near(sum(run(1.0).durations), 8, "as it is");
  near(sum(run(0).durations), 7, "none");
  const end = retime(film({ marks: [mark("hold", 6, 10, { kind: "end" })], beats }), [{ beat: "b", hold: 4 }]);
  near(sum(end.durations), 6 + 3 + 1, "the end hold of 3 s, set to 4");
});

test("an edit list that cannot be done is an error, never ignored", () => {
  const beats = [{ name: "a", t: 0 }, { name: "b", t: 5 }];
  const marks = [mark("hold", 1, 2, { kind: "after" })];
  const bad = [
    ["a beat that does not exist", [{ beat: "c", speed: 2 }], /beat "c" is not in the recording \(it has a, b\)/],
    ["no beat", [{ speed: 2 }], /beat "undefined"/],
    ["two changes in one entry", [{ beat: "a", cut: true, speed: 2 }], /exactly one of cut, speed and hold/],
    ["none", [{ beat: "a" }], /exactly one of cut, speed and hold \(this has none\)/],
    ["cut: false", [{ beat: "a", cut: false }], /"cut" is true/],
    ["speed 0", [{ beat: "a", speed: 0 }], /"speed" is a number above 0/],
    ["speed as text", [{ beat: "a", speed: "2" }], /"speed"/],
    ["from after to", [{ beat: "a", from: 2, to: 1, cut: true }], /before "to"/],
    ["a stretch past the end of its beat", [{ beat: "a", from: 1, to: 9, cut: true }], /ends at 9.0 s, and the beat "a" is 4.6 s long/],
    ["a key it does not know", [{ beat: "a", speed: 2, from_: 1 }], /"from_" is not a key/],
    ["from on a hold", [{ beat: "a", from: 1, hold: 1 }], /a hold has no "from"/],
    ["a hold in a beat that has none", [{ beat: "b", hold: 1 }], /beat "b" has no hold/],
    ["two stretches that overlap", [{ beat: "a", from: 1, to: 3, speed: 2 }, { beat: "a", from: 2, to: 4, cut: true }], /entries 1 and 2 overlap/],
    ["not an array", { beat: "a" }, /JSON array/],
    ["an entry that is not an object", ["a"], /an entry is an object/],
  ];
  for (const [what, edits, message] of bad) assert.throws(() => retime(film({ beats, marks }), edits, { file: "edits/g1.json" }), message, what);
  assert.throws(() => retime(film({ beats }), [{ beat: "zz", speed: 2 }], { file: "edits/g1.json" }), /edits\/g1\.json, entry 1/, "the error says which file and which entry");
});

test("the lines: what the automatic pass did and what the list did", () => {
  const r = retime(film({ marks: [mark("wait", 1, 4, { label: "Explore" })], beats: [{ name: "a", t: 0 }] }), [{ beat: "a", from: 5, to: 6, cut: true }], { label: "G1" });
  assert.match(r.lines[0], /^G1: recorded 10\.0 s → 7\.3 s \(the automatic pass\)$/);
  assert.match(r.lines[1], /^ {2}wait +1\.0 – 4\.0 +3\.0 s → 0\.3 s +\(Explore, cut to 0\.3 s\)$/);
  assert.match(r.lines[2], /the edit list: 1 entry, 7\.3 s → 6\.3 s/);
  assert.match(r.lines[3], /a 5\.0 – 6\.0: cut 1\.0 s/);
});

test("the edit list file: none is no edits; one that is not JSON says which file", () => {
  const dir = mkdtempSync(join(tmpdir(), "media-edits-"));
  assert.deepEqual(readEdits("g1", dir).entries, [], "no file, no manual edits");
  assert.equal(readEdits("g1", dir).text, null);
  writeFileSync(join(dir, "g1.json"), '[{ "beat": "end", "hold": 3 }]');
  assert.deepEqual(readEdits("g1", dir).entries, [{ beat: "end", hold: 3 }]);
  writeFileSync(join(dir, "g2.json"), "[{ nope");
  assert.throws(() => readEdits("g2", dir), /g2\.json is not JSON/);
});
