import assert from "node:assert/strict";
import { test } from "node:test";

import { inView, scrollFor } from "./viewport.ts";

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

// the table of the scroll: a row above, inside, below; at the ends; before the list has a height
const scrolls: { name: string; index: number; scrollTop: number; height: number; expect: number }[] = [
  { name: "a row already in view: nothing moves", index: 5, scrollTop: 0, height: 220, expect: 0 },
  { name: "a row above: its top goes to the top", index: 2, scrollTop: 220, height: 220, expect: 44 },
  { name: "a row below: its bottom goes to the bottom", index: 119, scrollTop: 0, height: 220, expect: 119 * 22 + 22 - 220 },
  { name: "a row just below the fold", index: 10, scrollTop: 0, height: 220, expect: 22 },
  { name: "half in at the bottom: it moves up just enough", index: 9, scrollTop: 11, height: 220, expect: 11 },
  { name: "half in at the top: it moves down just enough", index: 0, scrollTop: 11, height: 220, expect: 0 },
  { name: "a list with no height yet (the view is hidden): nothing to do", index: 50, scrollTop: 0, height: 0, expect: 0 },
  { name: "a node with no row: nothing to do", index: -1, scrollTop: 66, height: 220, expect: 66 },
];
for (const t of scrolls) test(`scrollFor: ${t.name}`, () => assert.equal(scrollFor(t.index, t.scrollTop, t.height, 22), t.expect));

test("whatever scrollFor answers, the row is then in view (when the list has a height)", () => {
  for (const index of [0, 1, 9, 10, 50, 119]) {
    for (const scrollTop of [0, 11, 100, 2000]) {
      const next = scrollFor(index, scrollTop, 220, 22);
      assert.equal(inView(index, next, 220, 22), true, `row ${index} from ${scrollTop} → ${next}`);
    }
  }
});
