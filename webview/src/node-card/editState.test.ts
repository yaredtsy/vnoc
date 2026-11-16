import assert from "node:assert/strict";
import { test } from "node:test";

import { editStateOf } from "./editState.ts";

test("a card's edit state: its draft's, else reading when the code is shown, else closed", () => {
  assert.equal(editStateOf(false, undefined), "closed");
  assert.equal(editStateOf(true, undefined), "reading");
  for (const state of ["editing", "saving", "conflict", "gone"] as const) {
    assert.equal(editStateOf(true, { state }), state);
    assert.equal(editStateOf(false, { state }), state, "a draft is shown whether or not the flag says the code is open");
  }
});
