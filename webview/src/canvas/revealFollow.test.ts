import assert from "node:assert/strict";
import test from "node:test";

import { FOLLOW_MS, MIN_READABLE_ZOOM, DOCS_SETTLE_MS, docsJustOpened, followsReveal, needsFraming, pickedCard, selectedDrawn } from "./revealFollow.ts";

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

test("the docs panel narrows a 1200 px pane by 380: a card the panel now covers is framed, one still whole is not moved", () => {
  const wide = { width: 920, height: 600 }; // the canvas beside a 280 px side bar
  const narrow = { width: wide.width - 380, height: 600 }; // with the panel (docWidth 380)
  const view = { x: 0, y: 0, zoom: 0.8 };
  const covered = { x: 500, y: 100, width: 400, height: 300 }; // right edge at 0.8 × 900 = 720: whole in the wide pane, under the panel in the narrow one
  const whole = { x: 100, y: 100, width: 400, height: 300 }; // right edge at 400
  assert.equal(needsFraming(covered, wide, view), false, "whole before the panel");
  assert.equal(needsFraming(covered, narrow, view), true, "the card the note is about is under the panel: framed");
  assert.equal(needsFraming(whole, narrow, view), false, "still whole beside the panel: nothing moves");
  assert.equal(needsFraming(whole, narrow, { x: 0, y: 0, zoom: 0.4 }), true, "whole, but under the readable zoom: framed as a reveal frames it");
});

test("the panel's opening is a change from closed to open; the card it opened for is the selected one, when drawn", () => {
  const drawn = (id: string) => id !== "gone";
  assert.equal(docsJustOpened(false, true), true);
  assert.equal(docsJustOpened(true, true), false, "already open: a later change of the pane is the person's");
  assert.equal(docsJustOpened(true, false), false, "closing");
  assert.equal(docsJustOpened(false, false), false);
  assert.equal(selectedDrawn("run_safe", drawn), "run_safe");
  assert.equal(selectedDrawn(null, drawn), null, "nothing selected");
  assert.equal(selectedDrawn("gone", drawn), null, "the selection is not a drawn card");
});

test("the card is judged after the selection has arrived, and inside the window a reveal's pane change is followed in", () => {
  assert.ok(DOCS_SETTLE_MS > 0 && DOCS_SETTLE_MS < FOLLOW_MS);
  assert.equal(followsReveal(1000, 1000 + DOCS_SETTLE_MS + 400), true, "a late change of the pane, after the card was judged");
});

test("a pick finds the first call card, in reading order, whose callee is the node, and none when the view draws no call card for it", () => {
  const cards = [
    { id: "a.py::run", role: "node", key: "a.py::run" },
    { id: "c1", role: "call", key: "lib.py::helper" },
    { id: "c2", role: "call", key: "lib.py::go" },
    { id: "c3", role: "call", key: "lib.py::go" }, // the same callee under another call: the first is the one
  ];
  assert.equal(pickedCard(cards, "lib.py::go"), "c2");
  assert.equal(pickedCard(cards, "lib.py::helper"), "c1");
  assert.equal(pickedCard(cards, "a.py::run"), null, "a structure card is not a call card: the pick reveals it by structure");
  assert.equal(pickedCard(cards, "lib.py::absent"), null);
  assert.equal(pickedCard([], "lib.py::go"), null);
});
