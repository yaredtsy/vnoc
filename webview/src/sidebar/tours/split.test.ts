import assert from "node:assert/strict";
import { test } from "node:test";

import { MIN_PANEL } from "../splitters.ts";
import { DEFAULT_TOURS_SHARE, dragTours, toursHeight, toursShare } from "./split.ts";

test("the section is 45% of the view until it is dragged", () => {
  assert.equal(DEFAULT_TOURS_SHARE, 0.45);
  assert.equal(toursHeight(800, DEFAULT_TOURS_SHARE), 360);
});

test("it is never under the panel minimum, and the tree above is never left under it", () => {
  assert.equal(toursHeight(800, 0.01), MIN_PANEL);
  assert.equal(toursHeight(800, 0.99), 800 - MIN_PANEL);
  assert.equal(toursHeight(200, 0.5), MIN_PANEL, "a view too small for both: the section keeps its minimum and the view scrolls");
});

test("dragging the splitter down makes the section smaller, up makes it taller, and a drag stops at the limits", () => {
  assert.equal(dragTours(800, 360, 100), 260);
  assert.equal(dragTours(800, 360, -100), 460);
  assert.equal(dragTours(800, 360, 5000), MIN_PANEL);
  assert.equal(dragTours(800, 360, -5000), 800 - MIN_PANEL);
});

test("the share kept is the height's share of the view", () => {
  assert.equal(toursShare(800, 400), 0.5);
  assert.equal(toursShare(0, 400), DEFAULT_TOURS_SHARE, "no height known yet");
  assert.equal(toursHeight(1000, toursShare(800, 400)), 500, "the same share in a taller view");
});
