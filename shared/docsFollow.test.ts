import assert from "node:assert/strict";
import test from "node:test";

import { type CallFacts, callId } from "./calls.ts";
import { docsNode, firstSource, followSource } from "./docsFollow.ts";
import type { TreeNode } from "./messages";

const node = (id: string, children: TreeNode[] = []): TreeNode => ({ id, name: id.split("::").pop() ?? id, kind: "function", children }) as unknown as TreeNode;
const checkout = node("app/api.py::checkout");
const reserve = node("app/inventory.py::reserve_stock");
const root = node("root", [checkout, reserve]);

const CALL = callId("app/api.py::checkout", ["app/inventory.py::reserve_stock"]);
const UNKNOWN = callId("app/api.py::checkout", ["?lib.py::mystery"]);
const facts = (id: string): CallFacts | null =>
  id === CALL
    ? { callee: { key: "app/inventory.py::reserve_stock", name: "reserve_stock" }, callerName: "checkout", lines: [3] }
    : id === UNKNOWN
      ? { callee: { key: null, name: "mystery" }, callerName: "checkout", lines: [4] }
      : null;

const none = { cursor: null, selected: null, source: "cursor" as const };

test("both change in one snapshot (a new tab's first): the cursor wins when it is set", () => {
  assert.equal(followSource(none, { cursor: "app/api.py::checkout", selected: CALL }), "cursor");
});

test("both change at once and there is no cursor: the selection", () => {
  assert.equal(followSource(none, { cursor: null, selected: CALL }), "selection");
});

test("only the selection changes: the selection; only the cursor changes: the cursor", () => {
  const was = { cursor: "app/api.py::checkout", selected: null, source: "cursor" as const };
  assert.equal(followSource(was, { cursor: "app/api.py::checkout", selected: CALL }), "selection");
  assert.equal(followSource({ ...was, selected: CALL, source: "selection" }, { cursor: "app/inventory.py::reserve_stock", selected: CALL }), "cursor");
});

test("nothing changes: the source stays", () => {
  const was = { cursor: "app/api.py::checkout", selected: CALL, source: "selection" as const };
  assert.equal(followSource(was, { cursor: was.cursor, selected: was.selected }), "selection");
});

test("the first source is the same rule: a selection alone, else the cursor", () => {
  assert.equal(firstSource({ cursor: null, selected: CALL }), "selection");
  assert.equal(firstSource({ cursor: "app/api.py::checkout", selected: CALL }), "cursor");
  assert.equal(firstSource({ cursor: null, selected: null }), "cursor");
});

const pick = (over: Partial<Parameters<typeof docsNode>[0]>) => docsNode({ pinned: null, source: "selection", selected: null, cursorNode: checkout, root, facts, ...over })?.id;

test("the pin beats both", () => {
  assert.equal(pick({ pinned: reserve, source: "cursor" }), "app/inventory.py::reserve_stock");
  assert.equal(pick({ pinned: reserve, selected: "app/api.py::checkout" }), "app/inventory.py::reserve_stock");
});

test("a selected call card with a known callee: the callee", () => {
  assert.equal(pick({ selected: CALL, cursorNode: null }), "app/inventory.py::reserve_stock");
});

test("a selected call card with an unknown callee, or one whose tree is not held: the cursor node, never empty", () => {
  assert.equal(pick({ selected: UNKNOWN }), "app/api.py::checkout");
  assert.equal(pick({ selected: callId("app/api.py::checkout", ["x"]) }), "app/api.py::checkout");
  assert.equal(pick({ selected: CALL, facts: () => null }), "app/api.py::checkout");
});

test("a selected node: that node; one that left the tree: the cursor node", () => {
  assert.equal(pick({ selected: "app/inventory.py::reserve_stock" }), "app/inventory.py::reserve_stock");
  assert.equal(pick({ selected: "app/gone.py::f" }), "app/api.py::checkout");
});

test("the cursor as source: the cursor node, whatever is selected", () => {
  assert.equal(pick({ source: "cursor", selected: CALL }), "app/api.py::checkout");
  assert.equal(pick({ source: "cursor", cursorNode: null }), undefined);
});
