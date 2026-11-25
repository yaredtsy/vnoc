import assert from "node:assert/strict";
import { test } from "node:test";

import { tourKey } from "./keys.ts";

test("the arrows move the row focus and stop at the ends; Home and End go to them", () => {
  assert.deepEqual(tourKey("ArrowDown", -1, 4, "list"), { type: "focus", index: 0 });
  assert.deepEqual(tourKey("ArrowDown", 1, 4, "list"), { type: "focus", index: 2 });
  assert.deepEqual(tourKey("ArrowDown", 3, 4, "list"), { type: "focus", index: 3 });
  assert.deepEqual(tourKey("ArrowUp", 2, 4, "route"), { type: "focus", index: 1 });
  assert.deepEqual(tourKey("ArrowUp", 0, 4, "route"), { type: "focus", index: 0 });
  assert.deepEqual(tourKey("ArrowUp", -1, 4, "route"), { type: "focus", index: 0 });
  assert.deepEqual([tourKey("Home", 2, 4, "list"), tourKey("End", 2, 4, "list")], [{ type: "focus", index: 0 }, { type: "focus", index: 3 }]);
});

test("Enter and Space open the focused row, and nothing when no row has the focus", () => {
  for (const mode of ["list", "route"] as const) {
    assert.deepEqual(tourKey("Enter", 1, 4, mode), { type: "open" });
    assert.deepEqual(tourKey(" ", 1, 4, mode), { type: "open" });
    assert.equal(tourKey("Enter", -1, 4, mode), null);
  }
});

test("Escape goes back to the list from the route, and is not ours in the list; other keys are not ours", () => {
  assert.deepEqual(tourKey("Escape", 1, 4, "route"), { type: "list" });
  assert.equal(tourKey("Escape", 1, 4, "list"), null);
  assert.equal(tourKey("x", 1, 4, "list"), null);
  assert.deepEqual(tourKey("Escape", -1, 0, "route"), { type: "list" }, "an empty route still goes back");
  assert.equal(tourKey("ArrowDown", -1, 0, "list"), null);
});
