import assert from "node:assert/strict";
import { test } from "node:test";

import { jump, pop, prune, push } from "./focusStack.ts";

test("push adds to the top, and the same node twice in a row is one entry", () => {
  assert.deepEqual(push(push([], "a"), "b"), ["a", "b"]);
  assert.deepEqual(push(["a"], "a"), ["a"]);
  assert.deepEqual(push(["a", "b"], "a"), ["a", "b", "a"], "going back to an earlier node is a new level, not a pop");
});

test("pop goes back one level, and popping nothing is fine", () => {
  assert.deepEqual(pop(["a", "b"]), ["a"]);
  assert.deepEqual(pop([]), []);
});

test("jump keeps the first n entries (a crumb click), 0 is the whole tree", () => {
  assert.deepEqual(jump(["a", "b", "c"], 1), ["a"]);
  assert.deepEqual(jump(["a", "b"], 0), []);
  assert.deepEqual(jump(["a"], 5), ["a"]);
});

test("a vanished node on top is dropped, down to one that still exists", () => {
  const alive = new Set(["a"]);
  assert.deepEqual(prune(["a", "b", "c"], (id) => alive.has(id)), ["a"]);
  assert.deepEqual(prune(["x"], (id) => alive.has(id)), []);
  const same = ["a"];
  assert.equal(prune(same, (id) => alive.has(id)), same, "nothing to drop returns the same array, so React sees no change");
});
