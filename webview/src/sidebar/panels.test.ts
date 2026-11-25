import assert from "node:assert/strict";
import { test } from "node:test";

import { callId } from "@vnoc/shared/calls.ts";
import type { Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { type ContextState, MAIN, type Session, emptySession, exploreId } from "@vnoc/shared/session.ts";
import { callerName, panelsOf, tabTarget } from "./panels.ts";

const n = (id: string, kind: Kind, name: string, children: TreeNode[] = []): TreeNode => ({ id, kind, name, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
const tree = n("/", "folder", "p", [
  n("app/api.py", "file", "api.py", [n("app/api.py::checkout", "function", "checkout")]),
  n("app/order.py", "file", "order.py", [n("app/order.py::checkout_order", "function", "checkout_order"), n("app/order.py::reserve", "function", "reserve")]),
]);

const C1 = callId("app/api.py::checkout", ["app/order.py::checkout_order"]); // checkout calls checkout_order
const C2 = callId("app/api.py::checkout", ["app/order.py::checkout_order", "app/order.py::reserve"]); // checkout_order calls reserve
const explore = (call: string, title: string, parent: string, over: Partial<ContextState> = {}): ContextState => ({ id: exploreId(call), title, kind: "explore", parent, call, tour: null, preview: false, focus: [], selected: null, open: {}, recent: [], ...over });
const session = (active: string, contexts: ContextState[]): Pick<Session, "order" | "contexts" | "active"> => {
  const main = emptySession().contexts[MAIN];
  return { order: [MAIN, ...contexts.map((c) => c.id)], contexts: Object.fromEntries([main, ...contexts].map((c) => [c.id, c])), active };
};

test("with one context there is one panel and no header", () => {
  const { panels, headers } = panelsOf(session(MAIN, []), tree);
  assert.deepEqual(panels.map((p) => [p.id, p.label, p.depth, p.active, p.closable]), [[MAIN, "MAIN", 0, true, false]]);
  assert.equal(headers, false);
});

test("two or more contexts: headers; MAIN, then the EXPLORE under its parent, with the words of 06 §4", () => {
  const { panels, headers } = panelsOf(session(MAIN, [explore(C1, "checkout_order", MAIN, { preview: true })]), tree, () => [16]);
  assert.equal(headers, true);
  assert.deepEqual(panels.map((p) => p.id), [MAIN, exploreId(C1)]);
  const e = panels[1];
  assert.equal(e.label, "◇ EXPLORE: checkout_order · from checkout");
  assert.equal(e.tooltip, "checkout calls checkout_order at line 16");
  assert.deepEqual([e.kind, e.depth, e.active, e.preview, e.closable], ["explore", 1, false, true, true]);
  assert.deepEqual([panels[0].active, panels[0].preview, panels[0].closable, panels[0].tooltip], [true, false, false, null]);
});

test("the tooltip names every line of a call made in several places, and none when the tree is not held", () => {
  const s = session(MAIN, [explore(C1, "checkout_order", MAIN)]);
  assert.equal(panelsOf(s, tree, () => [16, 31]).panels[1].tooltip, "checkout calls checkout_order at lines 16, 31");
  assert.equal(panelsOf(s, tree, () => null).panels[1].tooltip, "checkout calls checkout_order");
  assert.equal(panelsOf(s, tree, () => []).panels[1].tooltip, "checkout calls checkout_order");
});

test("an EXPLORE of an EXPLORE is a depth-2 panel under its parent, from the function the parent shows; the active one is marked", () => {
  const e1 = explore(C1, "checkout_order", MAIN);
  const e2 = explore(C2, "reserve", e1.id);
  const { panels } = panelsOf(session(e2.id, [e1, e2]), tree);
  assert.deepEqual(panels.map((p) => [p.label, p.depth, p.active]), [["MAIN", 0, false], ["◇ EXPLORE: checkout_order · from checkout", 1, false], ["◇ EXPLORE: reserve · from checkout_order", 2, true]]);
});

test("the panels follow `order`, which puts a child right under its parent", () => {
  const e1 = explore(C1, "checkout_order", MAIN);
  const e2 = explore(C2, "reserve", e1.id);
  const s = session(MAIN, [e1, e2]);
  assert.deepEqual(panelsOf({ ...s, order: [MAIN, e1.id, e2.id] }, tree).panels.map((p) => p.id), [MAIN, e1.id, e2.id]);
  assert.deepEqual(panelsOf({ ...s, order: [MAIN, e2.id, e1.id] }, tree).panels.map((p) => p.id), [MAIN, e2.id, e1.id], "it draws what the session says; the session keeps the order sound");
  assert.deepEqual(panelsOf({ ...s, order: [MAIN, "gone", e1.id] }, tree).panels.map((p) => p.id), [MAIN, e1.id], "an id with no context draws nothing");
});

test("who calls: the owner for a first step, the step before for a later one, the end of the key when the index does not have it", () => {
  assert.equal(callerName(C1, tree), "checkout");
  assert.equal(callerName(C2, tree), "checkout_order");
  assert.equal(callerName(callId("app/api.py::checkout", ["app/order.py::reserve#2"]), tree), "checkout");
  assert.equal(callerName(callId("x.py::gone", ["x.py::other"]), tree), "gone", "not in the tree: the end of its key");
  assert.equal(callerName("not a call", tree), "?");
});

// the table of 06 §7: Tab from a panel's tree to the next panel's tree
const order = [MAIN, "e1", "e2"];
const tabs: { name: string; from: string; shift: boolean; expect: string | null }[] = [
  { name: "Tab from MAIN goes to the next panel", from: MAIN, shift: false, expect: "e1" },
  { name: "Tab from the middle goes on", from: "e1", shift: false, expect: "e2" },
  { name: "Tab from the last panel leaves the view (the browser's)", from: "e2", shift: false, expect: null },
  { name: "Shift+Tab from the last goes back", from: "e2", shift: true, expect: "e1" },
  { name: "Shift+Tab from the first leaves the view", from: MAIN, shift: true, expect: null },
  { name: "a panel that is not there", from: "gone", shift: false, expect: null },
];
for (const t of tabs) test(`Tab: ${t.name}`, () => assert.equal(tabTarget(order, t.from, t.shift), t.expect));
test("Tab with one panel leaves the view", () => assert.equal(tabTarget([MAIN], MAIN, false), null));
