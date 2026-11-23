import assert from "node:assert/strict";
import test from "node:test";

import { FOLLOW_MS, MIN_READABLE_ZOOM, followsReveal, needsFraming } from "./revealFollow.ts";

const pane = { width: 900, height: 400 };
const card = { x: 100, y: 96, width: 300, height: 80 };

test("a card that is whole in the pane at a readable zoom is left where it is", () => {
  assert.equal(needsFraming(card, pane, { x: 0, y: 0, zoom: 1 }), false, "100 %, inside");
  assert.equal(needsFraming(card, pane, { x: 20, y: 30, zoom: 0.8 }), false, "80 %, inside");
  assert.equal(needsFraming(card, pane, { x: 0, y: 0, zoom: MIN_READABLE_ZOOM }), false, "exactly the smallest readable zoom");
});

test("a card that touches the pane's edge is whole; one pixel over is not", () => {
  assert.equal(needsFraming({ x: 0, y: 0, width: 900, height: 400 }, pane, { x: 0, y: 0, zoom: 1 }), false, "the pane's own size");
  assert.equal(needsFraming({ x: 0, y: 0, width: 902, height: 400 }, pane, { x: 0, y: 0, zoom: 1 }), true);
  assert.equal(needsFraming(card, pane, { x: 0.3, y: 0, zoom: 1 }), false);
});

test("a card cut by any edge, or off the pane, is framed", () => {
  assert.equal(needsFraming(card, pane, { x: -150, y: 0, zoom: 1 }), true, "cut at the left");
  assert.equal(needsFraming(card, pane, { x: 0, y: -100, zoom: 1 }), true, "cut at the top");
  assert.equal(needsFraming(card, pane, { x: 600, y: 0, zoom: 1 }), true, "cut at the right");
  assert.equal(needsFraming(card, pane, { x: 0, y: 300, zoom: 1 }), true, "cut at the bottom");
  assert.equal(needsFraming(card, pane, { x: 5000, y: 5000, zoom: 1 }), true, "far away");
});

test("below the smallest readable zoom the card is framed even when it is whole", () => {
  assert.equal(needsFraming(card, pane, { x: 0, y: 0, zoom: 0.175 }), true, "a column of 21 cards fitted: 17.5 %");
  assert.equal(needsFraming(card, pane, { x: 0, y: 0, zoom: 0.49 }), true);
});

test("a pane that has no size yet, or a zoom that is not one, is not known: the card is framed", () => {
  assert.equal(needsFraming(card, { width: 0, height: 0 }, { x: 0, y: 0, zoom: 1 }), true);
  assert.equal(needsFraming(card, pane, { x: 0, y: 0, zoom: 0 }), true);
});

test("the docs tab halves the pane: a card in its left part stays, one in the middle is framed again", () => {
  const half = { width: 454, height: 400 };
  assert.equal(needsFraming({ x: 104, y: 96, width: 300, height: 80 }, half, { x: 0, y: 0, zoom: 1 }), false, "N6's column, in the left 508 px");
  assert.equal(needsFraming({ x: 454, y: 96, width: 300, height: 80 }, half, { x: 0, y: 0, zoom: 1 }), true, "a card centred in the old pane");
});

test("the pane's change is followed just after a reveal, and not long after it or with no reveal", () => {
  assert.equal(followsReveal(1000, 1000), true);
  assert.equal(followsReveal(1000, 1063), true);
  assert.equal(followsReveal(1000, 1000 + FOLLOW_MS), true);
  assert.equal(followsReveal(1000, 1000 + FOLLOW_MS + 1), false);
  assert.equal(followsReveal(null, 5), false);
  assert.equal(followsReveal(2000, 1000), false, "a clock that went back is not a reason to move the view");
});
