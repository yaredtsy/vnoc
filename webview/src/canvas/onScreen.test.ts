import assert from "node:assert/strict";
import { test } from "node:test";

import { CARD_W } from "./layout.ts";
import { type Box, onScreenIds } from "./onScreen.ts";

const card = (id: string, x: number, y: number, h = 80): Box => ({ id, x, y, h });
const size = { width: 1000, height: 800 };
const at = (x = 0, y = 0, zoom = 1) => ({ x, y, zoom });

test("a card inside the viewport is on screen; one a screen away is too (the margin); one two screens away is not", () => {
  const cards = [card("in", 100, 100), card("right", 1500, 100), card("far", 3500, 100), card("below", 100, 1500), card("farBelow", 100, 3000)];
  assert.deepEqual(onScreenIds(cards, at(), size), ["in", "right", "below"]);
});

test("panning moves what is on screen: the viewport's origin is where the flow's origin is on screen", () => {
  const cards = [card("a", 0, 0), card("b", 4000, 0)];
  assert.deepEqual(onScreenIds(cards, at(), size), ["a"]);
  assert.deepEqual(onScreenIds(cards, at(-4000, 0), size), ["b"], "panned right by 4000: b is where a was; a is more than a screen behind");
});

test("zoomed out, a screen covers more of the flow, so more cards are on screen; zoomed in, fewer", () => {
  const cards = [card("a", 0, 0), card("b", 5000, 0)];
  assert.deepEqual(onScreenIds(cards, at(0, 0, 0.2), size), ["a", "b"], "at 0.2 a screen is 5000 flow units wide");
  assert.deepEqual(onScreenIds(cards, at(0, 0, 2), size), ["a"]);
});

test("the margin is one screen: a card just inside it counts, just outside does not", () => {
  const justIn = card("in", 2000 - 1, 0);
  const justOut = card("out", 2000 + 1 + 0, 0);
  assert.deepEqual(onScreenIds([justIn, justOut], at(), size), ["in"], "the viewport is 0–1000 and its margin reaches 2000");
  const behind = card("behind", -1000 - CARD_W + 1, 0);
  assert.deepEqual(onScreenIds([behind], at(), size), ["behind"], "and 1000 behind it, counting the card's own width");
});

test("a canvas not measured yet has every card on screen; the order given is kept", () => {
  const cards = [card("b", 9000, 9000), card("a", 0, 0)];
  assert.deepEqual(onScreenIds(cards, at(), { width: 0, height: 0 }), ["b", "a"]);
  assert.deepEqual(onScreenIds(cards, at(), size), ["a"]);
});
