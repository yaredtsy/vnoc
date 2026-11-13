import assert from "node:assert/strict";
import { test } from "node:test";

import { warningOf } from "./warning.ts";

const ok = { truncated: false, failed: [] as unknown[] };

test("nothing is wrong: no icon", () => {
  assert.deepEqual(warningOf({ structure: ok, status: "ready", message: "" }), { level: "none", text: "" });
});

test("unread files are counted, in the singular too", () => {
  assert.equal(warningOf({ structure: { ...ok, failed: [1, 2, 3] }, status: "ready", message: "" }).text, "3 files could not be read");
  assert.equal(warningOf({ structure: { ...ok, failed: [1] }, status: "ready", message: "" }).text, "1 file could not be read");
});

test("a cut structure says so, with the unread files after it", () => {
  const w = warningOf({ structure: { truncated: true, failed: [1] }, status: "ready", message: "" });
  assert.deepEqual(w, { level: "warning", text: "Too many files: the structure is cut · 1 file could not be read" });
});

test("a failed refresh with a structure drawn is an error that wins over the warning", () => {
  const w = warningOf({ structure: { truncated: true, failed: [1] }, status: "error", message: "core down" });
  assert.deepEqual(w, { level: "error", text: "Could not read the code again: core down" });
});

test("a failed first read has no structure: the bar says nothing (the canvas does)", () => {
  assert.deepEqual(warningOf({ structure: null, status: "error", message: "core down" }), { level: "none", text: "" });
});
