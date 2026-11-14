import assert from "node:assert/strict";
import test from "node:test";

import { type CallFacts, callId } from "./calls.ts";
import { calledByText, docSubject } from "./docSubject.ts";
import type { TreeNode } from "./messages";
import { type ContextState, MAIN, emptySession, exploreId } from "./session.ts";

const node = (id: string, children: TreeNode[] = []): TreeNode => ({ id, name: id.split("::").pop() ?? id, kind: "function", children }) as unknown as TreeNode;
const tree = node("root", [node("app/api.py::checkout"), node("app/inventory.py::reserve_stock")]);

const CALL = callId("app/api.py::checkout", ["app/inventory.py::reserve_stock"]);
const facts = (lines: number[]) => (id: string): CallFacts | null =>
  id === CALL ? { callee: { key: "app/inventory.py::reserve_stock", name: "reserve_stock" }, callerName: "checkout_order", lines } : null;

const withContexts = (main: Partial<ContextState>, explore?: Partial<ContextState>, active = MAIN) => {
  const s = emptySession();
  const contexts: Record<string, ContextState> = { [MAIN]: { ...s.contexts[MAIN], ...main } };
  const order = [MAIN];
  if (explore) {
    const id = exploreId(CALL);
    contexts[id] = { id, title: "reserve_stock", kind: "explore", parent: MAIN, call: CALL, preview: false, focus: [], selected: null, open: {}, recent: [], ...explore };
    order.push(id);
  }
  return { ...s, contexts, order, active };
};

test("a selected structure node: that node, nobody calls it", () => {
  const s = withContexts({ selected: "app/api.py::checkout" });
  const subject = docSubject(s, tree, facts([16]));
  assert.equal(subject.node?.id, "app/api.py::checkout");
  assert.equal(subject.calledBy, null);
});

test("a selected call: its callee, and `called by checkout_order · line 16`; several lines say `lines`", () => {
  const s = withContexts({ selected: CALL, focus: ["app/api.py::checkout"] });
  const one = docSubject(s, tree, facts([16]));
  assert.equal(one.node?.id, "app/inventory.py::reserve_stock");
  assert.equal(calledByText(one.calledBy!), "called by checkout_order · line 16");
  assert.equal(calledByText(docSubject(s, tree, facts([16, 31])).calledBy!), "called by checkout_order · lines 16, 31");
});

test("nothing selected: the focused node; no focus either: nothing", () => {
  assert.equal(docSubject(withContexts({ focus: ["app/api.py::checkout"] }), tree, facts([16])).node?.id, "app/api.py::checkout");
  assert.equal(docSubject(withContexts({}), tree, facts([16])).node, null);
});

test("a selection that left the tree falls back to the focused node", () => {
  const s = withContexts({ selected: "app/gone.py::f", focus: ["app/api.py::checkout"] });
  assert.equal(docSubject(s, tree, facts([16])).node?.id, "app/api.py::checkout");
});

test("a call that is not known (no tree held, or an unknown callee) has no docs: never the focused node's", () => {
  const s = withContexts({ selected: CALL, focus: ["app/api.py::checkout"] });
  assert.deepEqual(docSubject(s, tree, () => null), { node: null, calledBy: null });
  assert.deepEqual(docSubject(s, tree, () => ({ callee: { key: null, name: "len" }, callerName: "checkout_order", lines: [3] })), { node: null, calledBy: null });
});

test("switching the active context switches the subject", () => {
  const s = withContexts({ selected: "app/api.py::checkout" }, { selected: "app/inventory.py::reserve_stock" });
  assert.equal(docSubject(s, tree, facts([16])).node?.id, "app/api.py::checkout");
  assert.equal(docSubject({ ...s, active: exploreId(CALL) }, tree, facts([16])).node?.id, "app/inventory.py::reserve_stock");
});
