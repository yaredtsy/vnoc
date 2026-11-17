import assert from "node:assert/strict";
import { test } from "node:test";

import { newest, outcomeOf, placeOf } from "./search.ts";

test("where a result is: the path, with the 1-based line of a definition", () => {
  assert.equal(placeOf({ path: "billing/invoice.py", line: null }), "billing/invoice.py");
  assert.equal(placeOf({ path: "billing/invoice.py", line: 0 }), "billing/invoice.py:1");
  assert.equal(placeOf({ path: "billing/invoice.py", line: 41 }), "billing/invoice.py:42");
});

test("three requests sent, their answers arriving 3, 1, 2: only 3 is drawn", () => {
  const numbering = newest();
  const [one, two, three] = [numbering.next(), numbering.next(), numbering.next()];
  assert.deepEqual([one, two, three], [1, 2, 3]);
  const drawn = [three, one, two].filter((n) => numbering.isNewest(n));
  assert.deepEqual(drawn, [3]);
});

test("answers arriving 1, 2, 3 after all three were sent: only 3 is newest", () => {
  const numbering = newest();
  const sent = [numbering.next(), numbering.next(), numbering.next()];
  assert.deepEqual(sent.map((n) => numbering.isNewest(n)), [false, false, true]);
});

test("an answer is newest only until a later request is sent: 1 and 2 were drawn before 2 and 3 went out", () => {
  const numbering = newest();
  const one = numbering.next();
  assert.equal(numbering.isNewest(one), true, "answered before the next key");
  const two = numbering.next();
  assert.equal(numbering.isNewest(one), false, "…and not any more once request 2 was sent");
  assert.equal(numbering.isNewest(two), true);
  const three = numbering.next();
  assert.deepEqual([one, two, three].map((n) => numbering.isNewest(n)), [false, false, true]);
});

test("drop: an answer that arrives after it is not drawn, and the next request works as before", () => {
  const numbering = newest();
  const one = numbering.next();
  numbering.drop();
  assert.equal(numbering.isNewest(one), false, "the box was emptied while it was on its way");
  const two = numbering.next();
  assert.equal(two, 2, "numbers keep counting");
  assert.equal(numbering.isNewest(one), false);
  assert.equal(numbering.isNewest(two), true);
  numbering.drop();
  assert.equal(numbering.isNewest(two), false);
  numbering.drop(); // twice is the same
  assert.equal(numbering.isNewest(numbering.next()), true);
});

test("a number nobody sent is not newest", () => {
  const numbering = newest();
  assert.equal(numbering.isNewest(1), false);
  assert.equal(numbering.isNewest(0), false);
});

test("a refused request clears what was drawn only when it is the newest; an older one's answer or refusal changes nothing", () => {
  const numbering = newest();
  const [one, two] = [numbering.next(), numbering.next()];
  assert.equal(outcomeOf(numbering, one, false), "ignore", "an older refusal");
  assert.equal(outcomeOf(numbering, one, true), "ignore", "an older answer");
  assert.equal(outcomeOf(numbering, two, true), "draw");
  assert.equal(outcomeOf(numbering, two, false), "clear", "the newest was refused: no rows, no dim");
  numbering.drop();
  assert.equal(outcomeOf(numbering, two, false), "ignore", "the box was emptied since");
});
