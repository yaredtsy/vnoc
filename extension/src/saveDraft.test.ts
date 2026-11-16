import assert from "node:assert/strict";
import { test } from "node:test";

import { decideSave, outcomeAfterApply } from "./saveDraft.ts";

// docs/17-edit-in-canvas.md §6, the table of the three cases; 9.3 has the first row, 9.4 adds the others.

const BASE = "def f():\n    return 1";

const rows: { name: string; facts: Parameters<typeof decideSave>[0]; expect: "write" | "conflict" }[] = [
  { name: "the version is the draft's and the range holds base: write", facts: { draftVersion: 4, version: 4, current: BASE, base: BASE }, expect: "write" },
  { name: "the version moved on, the function is as it was (an edit elsewhere): a conflict until 9.4 finds it again", facts: { draftVersion: 4, version: 5, current: BASE, base: BASE }, expect: "conflict" },
  { name: "the version moved on and the function changed: a conflict", facts: { draftVersion: 4, version: 5, current: "def f():\n    return 2", base: BASE }, expect: "conflict" },
  { name: "the version is the draft's but the range holds other text: a conflict, nothing is written over it", facts: { draftVersion: 4, version: 4, current: "x = 1", base: BASE }, expect: "conflict" },
  { name: "a version that went back (an undo to the draft's) with the same text is still the draft's version: write", facts: { draftVersion: 4, version: 4, current: BASE, base: BASE }, expect: "write" },
  { name: "an older version than the draft's: a conflict", facts: { draftVersion: 4, version: 3, current: BASE, base: BASE }, expect: "conflict" },
  { name: "line endings count: the same text with other endings is other text", facts: { draftVersion: 4, version: 4, current: BASE.replace("\n", "\r\n"), base: BASE }, expect: "conflict" },
];
for (const r of rows) test(`decideSave: ${r.name}`, () => assert.equal(decideSave(r.facts), r.expect));

test("outcomeAfterApply: refused is a conflict; applied and saved is saved; applied and not saved is written (E4)", () => {
  assert.equal(outcomeAfterApply(false, false), "conflict");
  assert.equal(outcomeAfterApply(false, true), "conflict", "a save that says yes after a refusal cannot be taken for a write");
  assert.equal(outcomeAfterApply(true, true), "saved");
  assert.equal(outcomeAfterApply(true, false), "written");
});
