import assert from "node:assert/strict";
import { test } from "node:test";

import { wantedCalls, wantedOnScreen } from "@vnoc/shared/calls.ts";
import type { TreeNode } from "@vnoc/shared/messages.ts";
import { MAIN, emptySession } from "@vnoc/shared/session.ts";
import { CARD_W, type Placed } from "./layout.ts";
import { type Box, cardsReport, onScreenIds } from "./onScreen.ts";

const card = (id: string, x: number, y: number, h = 80): Box => ({ id, x, y, w: CARD_W, h });
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

// what the page reports of a drawing: probes for the cards with no structure children, and every function and class card for the prefetch
const placed = (id: string, kind: Placed["kind"], childCount: number, role: Placed["role"] = "node") => ({ id, kind, childCount, role });
const treeNode = (id: string, kind: TreeNode["kind"], children: TreeNode[] = []): TreeNode => ({
  id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children,
});

test("cardsReport: probes go to the cards with no children; every function and class card on screen is in onScreen", () => {
  const cards = [placed("a.py", "file", 3), placed("K", "class", 1), placed("K.run", "function", 0), placed("f", "function", 0), placed("far", "function", 0), placed("call:x", "function", 0, "call")];
  const visible = new Set(["a.py", "K", "K.run", "f", "call:x"]);
  assert.deepEqual(cardsReport(cards, visible, null), { keys: ["K.run", "f"], onScreen: ["K", "K.run", "f"] }, "a file and a call card are neither; `far` is off screen");
  assert.deepEqual(cardsReport(cards, visible, "f").keys, ["f", "K.run"], "the selected one first");
});

test("the prefetch of a selected class that has methods and no __init__ survives what the page reports", () => {
  const tree = treeNode("/", "folder", [treeNode("a.py", "file", [treeNode("K", "class", [treeNode("K.run", "function")])])]);
  const main = emptySession().contexts[MAIN];
  const session = { ...emptySession(), contexts: { [MAIN]: { ...main, selected: "K" } } };
  const wanted = wantedCalls(session, tree, {});
  assert.deepEqual(wanted, [{ key: "K", reason: "prefetch" }], "K is an entry: a class with no __init__ of its own");
  const cards = [placed("a.py", "file", 1), placed("K", "class", 1), placed("K.run", "function", 0)];
  const visible = new Set(["a.py", "K", "K.run"]);
  assert.deepEqual(wantedOnScreen(wanted, cardsReport(cards, visible, "K").onScreen), wanted, "on screen: still asked for");
  assert.deepEqual(wantedOnScreen(wanted, cardsReport(cards, new Set(["a.py"]), "K").onScreen), [], "scrolled off: dropped");
});
