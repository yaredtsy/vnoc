import assert from "node:assert/strict";
import { test } from "node:test";

import { CODE_H, COL_W, ROW_H, layout } from "./layout.ts";
import type { Kind, TreeNode } from "../types.ts";

const n = (id: string, kind: Kind, children: TreeNode[] = [], syntax_error = false): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error, node_id: null, docs: 0, link: null, children });

// repo/ ├ pkg/ ├ a.py ├ class C ├ method m   and  repo/ ├ b.py
const tree = n("/", "folder", [n("pkg", "folder", [n("a.py", "file", [n("C", "class", [n("m", "function")])])]), n("b.py", "file", [n("f", "function")])]);

test("at first only the root is open: its children are drawn closed, one row each, depth to the right", () => {
  const l = layout(tree, new Set());
  assert.deepEqual(l.nodes.map((p) => p.id), ["/", "pkg", "b.py"]);
  assert.deepEqual(l.nodes.map((p) => [p.x, p.y]), [[0, 0], [COL_W, ROW_H], [COL_W, 2 * ROW_H]]);
  assert.equal(l.hidden, 4, "a.py, C, m and f are not drawn");
  assert.deepEqual(l.edges.map((e) => e.id), ["/->pkg", "/->b.py"]);
});

test("the root of the view is open even when it was never opened (a focused node)", () => {
  const focused = tree.children[0];
  assert.deepEqual(layout(focused, new Set()).nodes.map((p) => p.id), ["pkg", "a.py"]);
});

test("a closed node says how many children it hides", () => {
  const pkg = layout(tree, new Set()).nodes.find((p) => p.id === "pkg")!;
  assert.equal(pkg.collapsed, true);
  assert.equal(pkg.childCount, 1);
});

test("opening a node shows its children, and the rows stay in source order", () => {
  const l = layout(tree, new Set(["pkg", "a.py", "C"]));
  assert.deepEqual(l.nodes.map((p) => p.id), ["/", "pkg", "a.py", "C", "m", "b.py"]);
  assert.equal(l.hidden, 1);
  assert.equal(l.nodes.find((p) => p.id === "m")!.depth, 4);
  assert.equal(l.nodes.find((p) => p.id === "pkg")!.collapsed, false);
});

test("a leaf is never shown as collapsed, and syntax errors are carried through", () => {
  const t = n("/", "folder", [n("bad.py", "file", [], true)]);
  const p = layout(t, new Set()).nodes[1];
  assert.equal(p.collapsed, false);
  assert.equal(p.syntaxError, true);
});

test("ids stay unique, so a toggle always hits one node", () => {
  const l = layout(tree, new Set(["pkg", "a.py", "C", "b.py"]));
  assert.equal(new Set(l.nodes.map((p) => p.id)).size, l.nodes.length);
});

test("an open code view pushes the rows below it down: defs and files have code, folders do not", () => {
  const range = { start: { line: 0, character: 0 }, end: { line: 3, character: 0 } };
  const t = n("/", "folder", [n("a.py", "file", [{ ...n("f", "function"), range }, n("g", "function")])]);
  const l = layout(t, new Set(["a.py"]), new Set(["/", "f", "a.py"]));
  assert.deepEqual(l.nodes.map((p) => p.y), [0, ROW_H, 2 * ROW_H + CODE_H, 3 * ROW_H + 2 * CODE_H], "the file and f are open and taller, so what is below them starts lower");
  assert.deepEqual(l.nodes.map((p) => p.codeOpen), [false, true, true, false], "the root folder has no code, so asking for it does nothing");
});
