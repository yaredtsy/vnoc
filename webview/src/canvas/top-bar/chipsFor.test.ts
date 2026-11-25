import assert from "node:assert/strict";
import test from "node:test";

import { type CallFacts, callId } from "@vnoc/shared/calls.ts";
import { type ContextState, MAIN, emptySession, exploreId } from "@vnoc/shared/session.ts";
import { ONE_CHIP, chipsFor } from "./chipsFor.ts";

const OWNER = "app/api.py::checkout";
const callTo = (name: string) => callId(OWNER, [`app/x.py::${name}`]);

/** MAIN plus EXPLORE contexts for the named callees, in that order. */
function session(names: string[], active: string = MAIN) {
  const s = emptySession();
  const contexts: Record<string, ContextState> = { ...s.contexts };
  const order = [MAIN];
  for (const name of names) {
    const call = callTo(name);
    const id = exploreId(call);
    contexts[id] = { id, title: name, kind: "explore", parent: MAIN, call, tour: null, preview: false, focus: [], selected: null, open: {}, recent: [] };
    order.push(id);
  }
  return { ...s, contexts, order, active: active === MAIN ? MAIN : exploreId(callTo(active)) };
}

const facts = (call: string): CallFacts | null =>
  call === callTo("reserve_stock") ? { callee: { key: "k", name: "reserve_stock" }, callerName: "checkout_order", lines: [16] } : null;
const wide = 800;

test("one context: no chips; two or more: MAIN and one chip per EXPLORE, in order", () => {
  assert.equal(chipsFor(session([]), wide, facts), null);
  const chips = chipsFor(session(["reserve_stock", "charge"]), wide, facts);
  assert.equal(chips?.mode, "row");
  assert.deepEqual(chips?.mode === "row" && chips.shown.map((c) => c.label), ["Main", "reserve_stock", "charge"]);
});

test("the active chip is the only one filled; MAIN has no ✕, an EXPLORE chip has", () => {
  const chips = chipsFor(session(["reserve_stock", "charge"], "charge"), wide, facts);
  assert.ok(chips?.mode === "row");
  assert.deepEqual(chips.shown.map((c) => [c.label, c.active, c.closable]), [["Main", false, false], ["reserve_stock", false, true], ["charge", true, true]]);
});

test("tooltip: `Explore: reserve_stock, from checkout_order (line 16)`; several lines say `lines`; a call that is not known says only the name", () => {
  const chips = chipsFor(session(["reserve_stock", "charge"]), wide, facts);
  assert.ok(chips?.mode === "row");
  assert.equal(chips.shown[0].tip, "Main");
  assert.equal(chips.shown[1].tip, "Explore: reserve_stock, from checkout_order (line 16)");
  assert.equal(chips.shown[2].tip, "Explore: charge");
  const two = chipsFor(session(["reserve_stock"]), wide, () => ({ callee: { key: "k", name: "reserve_stock" }, callerName: "checkout_order", lines: [16, 31] }));
  assert.ok(two?.mode === "row");
  assert.equal(two.shown[1].tip, "Explore: reserve_stock, from checkout_order (lines 16, 31)");
});

test("three EXPLORE chips fit; four fold the oldest into one (`+1 ▾`), five fold two", () => {
  const three = chipsFor(session(["a", "b", "c"]), wide, facts);
  assert.ok(three?.mode === "row");
  assert.equal(three.folded.length, 0);
  const four = chipsFor(session(["a", "b", "c", "d"]), wide, facts);
  assert.ok(four?.mode === "row");
  assert.deepEqual(four.folded.map((c) => c.label), ["a"]);
  assert.deepEqual(four.shown.map((c) => c.label), ["Main", "b", "c", "d"]);
  const five = chipsFor(session(["a", "b", "c", "d", "e"]), wide, facts);
  assert.ok(five?.mode === "row");
  assert.deepEqual(five.folded.map((c) => c.label), ["a", "b"]);
});

test("the active chip is never folded: the next oldest goes instead", () => {
  const chips = chipsFor(session(["a", "b", "c", "d"], "a"), wide, facts);
  assert.ok(chips?.mode === "row");
  assert.deepEqual(chips.folded.map((c) => c.label), ["b"]);
  assert.deepEqual(chips.shown.map((c) => c.label), ["Main", "a", "c", "d"]);
});

test("under 360 px: one chip with the active context's name, and a menu of them all", () => {
  const chips = chipsFor(session(["reserve_stock", "charge"], "charge"), ONE_CHIP - 1, facts);
  assert.ok(chips?.mode === "one");
  assert.equal(chips.current.label, "charge");
  assert.deepEqual(chips.all.map((c) => c.label), ["Main", "reserve_stock", "charge"]);
  assert.equal(chipsFor(session(["reserve_stock"]), ONE_CHIP, facts)?.mode, "row", "360 itself is wide enough");
  assert.equal(chipsFor(session([]), 100, facts), null, "one context is no chips, however narrow");
});

test("a tour's context is a chip `▶ title` with a ✕ that is never folded away; its tip says whose tour it is", () => {
  const s = session(["a", "b", "c", "d"]);
  const tourId = "tour:t-1";
  const withTour = { ...s, contexts: { ...s.contexts, [tourId]: { ...s.contexts[MAIN], id: tourId, title: "checkout_order", kind: "tour" as const, parent: MAIN, tour: "t-1" } }, order: [...s.order, tourId], active: tourId };
  const chips = chipsFor(withTour, wide, facts);
  assert.equal(chips?.mode, "row");
  if (chips?.mode !== "row") return;
  const chip = chips.shown.find((c) => c.id === tourId)!;
  assert.deepEqual([chip.label, chip.closable, chip.kind, chip.active, chip.tip], ["▶ checkout_order", true, "tour", true, "Tour of checkout_order"]);
  assert.equal(chips.folded.some((c) => c.id === tourId), false, "only EXPLORE chips fold");
  assert.equal(chips.folded.length, 1, "four EXPLORE chips: the earliest one folds");
  // MAIN stays without a ✕
  assert.equal(chips.shown[0].closable, false);
});
