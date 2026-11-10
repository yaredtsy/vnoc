import assert from "node:assert/strict";
import { test } from "node:test";

import { CODE_H, COL_W, ROW_H, defaultCollapsed, layout } from "./layout.ts";
import type { Kind, TreeNode } from "../types.ts";

const n = (id: string, kind: Kind, children: TreeNode[] = [], syntax_error = false): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error, children });

// repo/ ├ pkg/ ├ a.py ├ class C ├ method m   and  repo/ ├ b.py
const tree = n("/", "folder", [n("pkg", "folder", [n("a.py", "file", [n("C", "class", [n("m", "function")])])]), n("b.py", "file", [n("f", "function")])]);

test("by default files are collapsed and folders from depth 2 down", () => {
  assert.deepEqual(defaultCollapsed(tree).sort(), ["a.py", "b.py"]);
  const deep = n("/", "folder", [n("a", "folder", [n("b", "folder", [n("c", "folder", [n("x.py", "file")])])])]);
  assert.deepEqual(defaultCollapsed(deep), ["b", "c"], "b (depth 2) and c (depth 3); x.py has no children, so nothing to collapse");
});

test("a collapsed tree draws one row per visible node, depth to the right", () => {
  const l = layout(tree, new Set(defaultCollapsed(tree)));
  assert.deepEqual(l.nodes.map((p) => p.id), ["/", "pkg", "a.py", "b.py"]);
  assert.deepEqual(l.nodes.map((p) => [p.x, p.y]), [[0, 0], [COL_W, ROW_H], [2 * COL_W, 2 * ROW_H], [COL_W, 3 * ROW_H]]);
  assert.equal(l.hidden, 3, "C, m and f are not drawn");
  assert.deepEqual(l.edges.map((e) => e.id), ["/->pkg", "pkg->a.py", "/->b.py"]);
});

test("a collapsed node says how many children it hides", () => {
  const a = layout(tree, new Set(["a.py", "b.py"])).nodes.find((p) => p.id === "a.py")!;
  assert.equal(a.collapsed, true);
  assert.equal(a.childCount, 1);
});

test("expanding shows the children, and the rows stay in source order", () => {
  const l = layout(tree, new Set(["b.py"]));
  assert.deepEqual(l.nodes.map((p) => p.id), ["/", "pkg", "a.py", "C", "m", "b.py"]);
  assert.equal(l.hidden, 1);
  assert.equal(l.nodes.find((p) => p.id === "m")!.depth, 4);
});

test("a leaf is never shown as collapsed, and syntax errors are carried through", () => {
  const t = n("/", "folder", [n("bad.py", "file", [], true)]);
  const p = layout(t, new Set(["bad.py"])).nodes[1];
  assert.equal(p.collapsed, false);
  assert.equal(p.syntaxError, true);
});

test("ids stay unique, so an expand toggle always hits one node", () => {
  const l = layout(tree, new Set());
  assert.equal(new Set(l.nodes.map((p) => p.id)).size, l.nodes.length);
});

test("an open code view pushes the rows below it down, and only defs have code", () => {
  const range = { start: { line: 0, character: 0 }, end: { line: 3, character: 0 } };
  const t = n("/", "folder", [n("a.py", "file", [{ ...n("f", "function"), range }, n("g", "function")])]);
  const l = layout(t, new Set(), new Set(["f", "a.py"]));
  assert.deepEqual(l.nodes.map((p) => p.y), [0, ROW_H, 2 * ROW_H, 3 * ROW_H + CODE_H], "f is open and taller, so g starts lower; a.py has no range, so asking for its code does nothing");
  assert.deepEqual(l.nodes.map((p) => p.codeOpen), [false, false, true, false]);
});
