import assert from "node:assert/strict";
import { test } from "node:test";

import { type NodeFacts, nodeContext, paneContext, panelContext } from "./nodeContext.ts";

const RANGE = { start: { line: 0, character: 0 }, end: { line: 3, character: 0 } };
const callCard = (over: Partial<NodeFacts> = {}, sites = 1): NodeFacts => ({
  id: 'call:["app/api.py::checkout","app/order.py::reserve"]', key: "app/order.py::reserve", role: "call", kind: "function", range: RANGE, codeOpen: false, docs: 0, link: null, childCount: 0, callsChevron: true, collapsed: true,
  call: { sites: Array.from({ length: sites }, () => ({})) }, ...over,
});
const ctx = (node: NodeFacts, contextId = "main") => JSON.parse(nodeContext({ node, contextId, isFocusRoot: false }));

// what a call card tells the menu (08 §6): the table of the call menu's `when` keys
const table: { name: string; node: NodeFacts; expect: Record<string, unknown> }[] = [
  { name: "a known callee called once, closed", node: callCard(), expect: { role: "call", calleeKnown: true, sites: 1, hasChildren: true, open: false, hasCode: true, codeOpen: false, nodeKey: "app/order.py::reserve" } },
  { name: "called from three places", node: callCard({}, 3), expect: { sites: 3 } },
  { name: "open", node: callCard({ collapsed: false }), expect: { open: true, hasChildren: true } },
  { name: "its code open in the card", node: callCard({ codeOpen: true }), expect: { codeOpen: true } },
  { name: "a callee the project does not know: no node to act on, and no code", node: callCard({ key: null, range: null, callsChevron: false }), expect: { calleeKnown: false, nodeKey: null, hasCode: false, hasChildren: false, role: "call" } },
  { name: "a call with nothing under it", node: callCard({ callsChevron: false }), expect: { hasChildren: false, open: false } },
];
for (const t of table) {
  test(`nodeContext of a call card: ${t.name}`, () => {
    const c = ctx(t.node);
    for (const [key, value] of Object.entries(t.expect)) assert.deepEqual(c[key], value, key);
    assert.equal(c.webviewSection, "node", "a call card is a node section: its entries test `role == call`, so a node's never show on it");
    assert.equal(c.surface, "canvas");
    assert.equal(c.callId, t.node.id, "which call it is");
    assert.equal(c.id, t.node.id);
  });
}

test("a structure card says it is no call: no callId, calleeKnown true, no sites", () => {
  const c = ctx({ ...callCard(), id: "app/api.py::checkout", role: "node", call: null, callsChevron: false });
  assert.deepEqual([c.role, c.callId, c.calleeKnown, c.sites], ["node", undefined, true, 0]);
});

test("the pane and the panel header: their own sections, the surface, and the context they are about", () => {
  assert.deepEqual(JSON.parse(paneContext("canvas")), { surface: "canvas", webviewSection: "pane", preventDefaultContextMenuItems: true });
  assert.equal(JSON.parse(paneContext("sidebar")).surface, "sidebar");
  assert.deepEqual(JSON.parse(panelContext({ id: "explore:x", kind: "explore" })), { surface: "sidebar", webviewSection: "context", preventDefaultContextMenuItems: true, contextId: "explore:x", contextKind: "explore" });
  assert.equal(JSON.parse(panelContext({ id: "main", kind: "main" })).contextKind, "main");
});
