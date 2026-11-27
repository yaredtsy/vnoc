import assert from "node:assert/strict";
import { test } from "node:test";

import { moreLit, nextPage, scrollTarget, viewOf } from "./scroll.ts";

const view = (top: number, rows = 9) => ({ top, rows });

test("a new stop starts at the top, whatever the pointer does", () => {
  assert.equal(scrollTarget({ newStop: true, lit: [20, 22], view: view(15), pointerOver: false }), 0);
  assert.equal(scrollTarget({ newStop: true, lit: null, view: view(15), pointerOver: true }), 0);
  assert.equal(scrollTarget({ newStop: true, lit: null, view: view(0), pointerOver: false }), null, "already there");
});

test("a new step in the same stop scrolls only when its lit lines are not fully in view", () => {
  assert.equal(scrollTarget({ newStop: false, lit: [2, 6], view: view(0), pointerOver: false }), null, "fully in view: the code did not scroll");
  assert.equal(scrollTarget({ newStop: false, lit: [0, 8], view: view(0), pointerOver: false }), null, "the last line in view counts");
  assert.equal(scrollTarget({ newStop: false, lit: [9, 12], view: view(0), pointerOver: false }), 7, "its first line goes two lines under the top");
  assert.equal(scrollTarget({ newStop: false, lit: [3, 10], view: view(0), pointerOver: false }), 1, "its end is out of view");
  assert.equal(scrollTarget({ newStop: false, lit: [1, 3], view: view(5), pointerOver: false }), 0, "above the pane: back up, not below zero");
  assert.equal(scrollTarget({ newStop: false, lit: [0, 1], view: view(5), pointerOver: false }), 0);
});

test("a step with nothing lit does not scroll, and neither does one while the pointer is over the pane", () => {
  assert.equal(scrollTarget({ newStop: false, lit: null, view: view(4), pointerOver: false }), null);
  assert.equal(scrollTarget({ newStop: false, lit: [30, 32], view: view(0), pointerOver: true }), null);
});

test("it does not report a scroll to where the pane already is", () => {
  assert.equal(scrollTarget({ newStop: false, lit: [9, 12], view: view(7), pointerOver: false }), null, "in view after the move: nothing to do");
  assert.equal(scrollTarget({ newStop: false, lit: [12, 30], view: view(10), pointerOver: false }), null, "starts two under the top already, and is taller than the pane");
});

test("a block taller than the pane says how many lit lines are below it", () => {
  assert.equal(moreLit([2, 20], view(0)), 12, "lines 9 to 20 are below the bottom line 8");
  assert.equal(moreLit([2, 6], view(0)), 0);
  assert.equal(moreLit([0, 8], view(0)), 0, "the last lit line is the last one in view");
  assert.equal(moreLit([12, 20], view(0)), 9, "a block that starts below the pane is all below it");
  assert.equal(moreLit(null, view(0)), 0);
  assert.equal(nextPage(view(0), 40), 9);
  assert.equal(nextPage(view(36), 40), 39, "never past the last line");
});

test("a pane with wrapped lines: the view is the first line more than half in view, and the lines that are whole from it", () => {
  // five lines of 18 px, the second one wrapped onto three rows (54 px): tops 0 18 72 90 108, ends at 126
  const tops = [0, 18, 72, 90, 108];
  const heights = [18, 54, 18, 18, 18];
  assert.deepEqual(viewOf(tops, heights, 0, 90), { top: 0, rows: 3 }, "lines 0 to 2 end at 90");
  assert.deepEqual(viewOf(tops, heights, 18, 90), { top: 1, rows: 3 }, "from 18 to 108: the wrapped line, and the two after it");
  assert.deepEqual(viewOf(tops, heights, 40, 36), { top: 1, rows: 1 }, "a line taller than what is in view still counts as the one");
  assert.deepEqual(viewOf(tops, heights, 200, 90), { top: 4, rows: 1 }, "past the end: the last line");
  // lines of one height are the old rule: the row is scrollTop / 18 rounded
  const same = Array.from({ length: 20 }, (_, i) => i * 18);
  assert.equal(viewOf(same, same.map(() => 18), 45, 162).top, 3);
  assert.equal(viewOf(same, same.map(() => 18), 36, 162).top, 2);
  assert.equal(viewOf(same, same.map(() => 18), 36, 162).rows, 9);
});
