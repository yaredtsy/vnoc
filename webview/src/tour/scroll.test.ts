import assert from "node:assert/strict";
import { test } from "node:test";

import { moreLit, nextPage, scrollTarget } from "./scroll.ts";

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
