import assert from "node:assert/strict";
import { test } from "node:test";

import { NARROW, type Step, foldCrumbs } from "./foldCrumbs.ts";

const steps = (n: number): Step[] => Array.from({ length: n }, (_, i) => ({ id: `s${i}`, label: `s${i}`, depth: i }));
const ids = (list: Step[]) => list.map((s) => s.id);

test("520 px is wide: nothing folds; 519 px is narrow", () => {
  assert.equal(NARROW, 520);
  assert.deepEqual(foldCrumbs(steps(5), 520).folded, []);
  assert.deepEqual(ids(foldCrumbs(steps(5), 519).folded), ["s0", "s1", "s2"]);
});

test("1 step and 2 steps: nothing to fold, even when narrow", () => {
  for (const n of [1, 2]) {
    const { folded, shown } = foldCrumbs(steps(n), 100);
    assert.deepEqual(folded, []);
    assert.equal(shown.length, n);
  }
});

test("5 steps when narrow: the last two stay, the other three fold", () => {
  const { folded, shown } = foldCrumbs(steps(5), 300);
  assert.deepEqual(ids(folded), ["s0", "s1", "s2"]);
  assert.deepEqual(ids(shown), ["s3", "s4"]);
});

test("the project (the first step) folds first", () => {
  assert.equal(foldCrumbs(steps(3), 300).folded[0].depth, 0);
  assert.deepEqual(ids(foldCrumbs(steps(3), 300).folded), ["s0"]);
});

test("each folded step jumps to its own depth", () => {
  assert.deepEqual(foldCrumbs(steps(5), 300).folded.map((s) => s.depth), [0, 1, 2]);
});

test("the current crumb (the last step) is never folded, at any width", () => {
  for (const width of [0, 100, 519, 520, 2000]) for (const n of [1, 2, 3, 6]) assert.equal(foldCrumbs(steps(n), width).shown.at(-1)!.id, `s${n - 1}`);
});

test("nothing is lost: folded then shown is the whole path in order", () => {
  const all = steps(6);
  const { folded, shown } = foldCrumbs(all, 200);
  assert.deepEqual([...folded, ...shown], all);
});
