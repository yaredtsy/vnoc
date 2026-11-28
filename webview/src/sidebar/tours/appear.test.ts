import assert from "node:assert/strict";
import { test } from "node:test";

import { appearedFromOutside } from "./appear.ts";

test("the section unfolds when a tour appears from outside while no tour is open", () => {
  assert.equal(appearedFromOutside(["a", "b"], ["c", "a", "b"], null), true);
  assert.equal(appearedFromOutside(["a", "b"], ["a", "b"], null), false, "the same list again");
  assert.equal(appearedFromOutside(["a", "b"], ["b"], null), false, "a tour that went");
  assert.equal(appearedFromOutside(null, ["a"], null), false, "the first list is not news");
  assert.equal(appearedFromOutside(["a"], ["b", "a"], "a"), false, "a tour is open: the window is busy with it");
  assert.equal(appearedFromOutside([], ["a"], null), true, "the first tour of an empty folder");
});
