import assert from "node:assert/strict";
import { test } from "node:test";

import { budgetNote } from "./budgetNote.ts";

test("everything drawn: no note", () => {
  assert.equal(budgetNote(40, 40), null);
  assert.equal(budgetNote(200, 200), null, "exactly the budget is not over it");
});

test("some held back: `200 of 640 cards`, and what to do about it; no Show all", () => {
  const note = budgetNote(200, 640)!;
  assert.equal(note.text, "200 of 640 cards");
  assert.equal(note.tip, "Some open cards are drawn closed to keep the canvas fast. Focus on a part to see all of it.");
});

test("a root drawn alone says so too", () => assert.equal(budgetNote(1, 201)?.text, "1 of 201 cards"));
