import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_SHARE, MIN_PANEL, dragSplitter, panelHeights, resetSplitter, sharesOf } from "./splitters.ts";

const H = 800;

// panelHeights: shares → heights, with the limits
const layouts: { name: string; total: number; shares: number[]; expect: number[]; scroll: boolean }[] = [
  { name: "one panel is the whole view", total: H, shares: [], expect: [800], scroll: false },
  { name: "a new EXPLORE panel takes 45%, MAIN the rest", total: H, shares: [DEFAULT_SHARE], expect: [440, 360], scroll: false },
  { name: "a saved size is kept", total: H, shares: [0.25], expect: [600, 200], scroll: false },
  { name: "a panel is never under the minimum", total: H, shares: [0.05], expect: [688, 112], scroll: false },
  { name: "MAIN is never under the minimum: the panel below gives room", total: H, shares: [0.95], expect: [112, 688], scroll: false },
  { name: "two EXPLORE panels at 45% each would starve MAIN: the last gives up room first", total: H, shares: [0.45, 0.45], expect: [112, 360, 328], scroll: false },
  { name: "a short view: both panels at their minimum add up to more than the view, so it scrolls", total: 200, shares: [DEFAULT_SHARE], expect: [112, 112], scroll: true },
  { name: "three panels that cannot fit scroll", total: 300, shares: [0.45, 0.45], expect: [112, 112, 112], scroll: true },
];
for (const l of layouts) test(`panelHeights: ${l.name}`, () => assert.deepEqual([panelHeights(l.total, l.shares).heights, panelHeights(l.total, l.shares).scroll], [l.expect, l.scroll]));

test("panelHeights: when it fits, the heights add up to the view", () => {
  for (const shares of [[0.45], [0.2, 0.3], [0.1, 0.1, 0.1]]) assert.equal(panelHeights(H, shares).heights.reduce((a, b) => a + b, 0), H, JSON.stringify(shares));
});

// dragSplitter: the panel below follows the splitter, the one above takes or gives the room, limits stop it, nothing else moves
const drags: { name: string; heights: number[]; index: number; dy: number; expect: number[] }[] = [
  { name: "up grows the panel below and shrinks MAIN", heights: [360, 440], index: 1, dy: -100, expect: [260, 540] },
  { name: "down shrinks the panel below and grows MAIN", heights: [360, 440], index: 1, dy: 100, expect: [460, 340] },
  { name: "up stops when MAIN reaches its minimum", heights: [360, 440], index: 1, dy: -1000, expect: [MIN_PANEL, 688] },
  { name: "down stops when the panel below reaches its minimum", heights: [360, 440], index: 1, dy: 1000, expect: [688, MIN_PANEL] },
  { name: "a drag of nothing moves nothing", heights: [360, 440], index: 1, dy: 0, expect: [360, 440] },
  { name: "the second splitter moves the first EXPLORE panel and the last, and not MAIN", heights: [200, 300, 300], index: 2, dy: -80, expect: [200, 220, 380] },
  { name: "…and stops at the first EXPLORE panel's minimum, not pushing MAIN", heights: [200, 300, 300], index: 2, dy: -1000, expect: [200, MIN_PANEL, 488] },
  { name: "the first splitter with two EXPLORE panels leaves the last alone", heights: [200, 300, 300], index: 1, dy: 50, expect: [250, 250, 300] },
  { name: "no splitter on MAIN (index 0) or past the end: nothing", heights: [360, 440], index: 0, dy: 50, expect: [360, 440] },
  { name: "panels already at their minimum do not move", heights: [MIN_PANEL, MIN_PANEL], index: 1, dy: -50, expect: [MIN_PANEL, MIN_PANEL] },
];
for (const d of drags) test(`dragSplitter: ${d.name}`, () => assert.deepEqual(dragSplitter(d.heights, d.index, d.dy), d.expect));

test("dragSplitter: the total never changes, and no panel ever goes under the minimum", () => {
  const start = [200, 300, 300];
  for (const index of [1, 2]) {
    for (let dy = -700; dy <= 700; dy += 35) {
      const r = dragSplitter(start, index, dy);
      assert.equal(r.reduce((a, b) => a + b, 0), 800, `index ${index} dy ${dy}`);
      assert.ok(r.every((h) => h >= MIN_PANEL), `index ${index} dy ${dy}: ${r}`);
    }
  }
});

test("a double-click goes back to 45%, as far as the limits allow", () => {
  assert.deepEqual(resetSplitter(H, [560, 240], 1), [440, 360]);
  assert.deepEqual(resetSplitter(H, [300, 500], 1), [440, 360]);
  assert.deepEqual(resetSplitter(H, [440, 360], 1), [440, 360], "already there");
  // with two EXPLORE panels the room comes from the panel above
  assert.deepEqual(resetSplitter(H, [112, 200, 488], 2), [112, 328, 360]);
});

test("the shares to keep: one per panel after the first, and they come back as the same heights", () => {
  const heights = [360, 440];
  const shares = sharesOf(H, heights);
  assert.deepEqual(shares, [0.55]);
  assert.deepEqual(panelHeights(H, shares).heights, heights);
  assert.deepEqual(sharesOf(0, [0, 0]), [DEFAULT_SHARE], "a view with no height yet keeps the default");
});
