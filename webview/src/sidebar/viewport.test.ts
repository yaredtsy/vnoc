import assert from "node:assert/strict";
import { test } from "node:test";

import { inView } from "./viewport.ts";

test("a row is in view when all of it is inside the scroll area", () => {
  // 22 px rows, a 220 px area: rows 0..9 at the top
  assert.equal(inView(0, 0, 220, 22), true);
  assert.equal(inView(9, 0, 220, 22), true);
  assert.equal(inView(10, 0, 220, 22), false, "the first row below");
  assert.equal(inView(10, 22, 220, 22), true, "scrolled by one row");
  assert.equal(inView(0, 22, 220, 22), false, "scrolled off the top");
  assert.equal(inView(0, 11, 220, 22), false, "half scrolled off the top: not all of it");
  assert.equal(inView(10, 0, 231, 22), false, "half below the bottom: not all of it");
  assert.equal(inView(-1, 0, 220, 22), false, "a node with no row is not in view");
});
