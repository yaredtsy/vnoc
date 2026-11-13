import assert from "node:assert/strict";
import { test } from "node:test";

import type { CallTreeResult } from "@vnoc/shared/messages.ts";
import { type CallEvent, type CallState, applyCallEvent, noCalls } from "./callEvents.ts";

const tree = (revision: number): CallTreeResult => ({ revision, root: { key: null, name: "x", qualname: "x", path: null, kind: "function", call_count: 1, call_sites: [], marks: [], children: [] } });
const held = (...keys: string[]): CallState => ({ trees: Object.fromEntries(keys.map((k) => [k, tree(1)])), states: Object.fromEntries(keys.map((k) => [k, { state: "idle" as const }])) });

// each row: the state before, the message, the keys of the trees and the states after
const table: { name: string; before: CallState; event: CallEvent; trees: string[]; states: string[] }[] = [
  { name: "a tree arrives", before: noCalls, event: { type: "tree", key: "a", result: tree(1) }, trees: ["a"], states: [] },
  { name: "a state arrives", before: noCalls, event: { type: "state", key: "a", state: "loading" }, trees: [], states: ["a"] },
  { name: "a tree for a key that is held replaces it", before: held("a"), event: { type: "tree", key: "a", result: tree(2) }, trees: ["a"], states: ["a"] },
  { name: "dropped removes the tree and what was known of its state", before: held("a", "b"), event: { type: "dropped", key: "a" }, trees: ["b"], states: ["b"] },
  { name: "dropped removes a state that has no tree (a failed key)", before: { trees: {}, states: { a: { state: "failed", message: "boom" } } }, event: { type: "dropped", key: "a" }, trees: [], states: [] },
  { name: "reset empties everything", before: held("a", "b"), event: { type: "reset" }, trees: [], states: [] },
  { name: "the snapshot adds the trees the host held when the page asked", before: noCalls, event: { type: "snapshot", trees: { a: tree(1), b: tree(1) } }, trees: ["a", "b"], states: [] },
];
for (const row of table) {
  test(`applyCallEvent: ${row.name}`, () => {
    const after = applyCallEvent(row.before, row.event);
    assert.deepEqual(Object.keys(after.trees).sort(), row.trees);
    assert.deepEqual(Object.keys(after.states).sort(), row.states);
  });
}

test("a tree replaces the older one, and the snapshot never replaces what an event already brought", () => {
  assert.equal(applyCallEvent(held("a"), { type: "tree", key: "a", result: tree(2) }).trees.a.revision, 2);
  const newer: CallState = { trees: { a: tree(5) }, states: {} };
  assert.equal(applyCallEvent(newer, { type: "snapshot", trees: { a: tree(1) } }).trees.a.revision, 5, "an event that came first is newer than the list");
});

test("a state carries its message, and failed then loading then idle each replace the one before", () => {
  const failed = applyCallEvent(noCalls, { type: "state", key: "a", state: "failed", message: "boom" });
  assert.deepEqual(failed.states.a, { state: "failed", message: "boom" });
  const loading = applyCallEvent(failed, { type: "state", key: "a", state: "loading" });
  assert.deepEqual(loading.states.a, { state: "loading", message: undefined });
});

test("a message that changes nothing returns the same object (the store does not draw again)", () => {
  const s = held("a");
  assert.equal(applyCallEvent(s, { type: "dropped", key: "ghost" }), s, "dropped: a key the page never had");
  assert.equal(applyCallEvent(s, { type: "state", key: "a", state: "idle" }), s, "the same state again");
  assert.equal(applyCallEvent(noCalls, { type: "reset" }), noCalls, "reset: nothing to empty");
});

test("reset then a tree from the new core: only the new one is there", () => {
  const after = [{ type: "reset" } as CallEvent, { type: "tree", key: "a", result: tree(3) } as CallEvent].reduce(applyCallEvent, held("a", "b"));
  assert.deepEqual(Object.keys(after.trees), ["a"]);
  assert.equal(after.trees.a.revision, 3);
});
