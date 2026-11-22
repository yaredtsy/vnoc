import assert from "node:assert/strict";
import test from "node:test";

import { FOLLOW_MS, followsReveal } from "./revealFollow.ts";

test("the pane changes size just after a reveal framed a card: the card is framed again", () => {
  assert.equal(followsReveal(1000, 1000), true);
  assert.equal(followsReveal(1000, 1063), true);
  assert.equal(followsReveal(1000, 1000 + FOLLOW_MS), true);
});

test("a change of the pane's size long after a reveal, or with no reveal, is left alone", () => {
  assert.equal(followsReveal(1000, 1000 + FOLLOW_MS + 1), false);
  assert.equal(followsReveal(null, 5), false);
  assert.equal(followsReveal(2000, 1000), false, "a clock that went back is not a reason to move the view");
});
