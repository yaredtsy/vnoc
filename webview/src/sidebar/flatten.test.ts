import assert from "node:assert/strict";
import { test } from "node:test";

import type { Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { flatten } from "./flatten.ts";

const n = (id: string, kind: Kind, children: TreeNode[] = []): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });

// /  ├ pkg ├ a.py ├ C ├ m            (the opened folder "/" is not a row)
//    │     └ b.py ├ f
//    └ top.py ├ g
const tree = n("/", "folder", [n("pkg", "folder", [n("a.py", "file", [n("C", "class", [n("m", "function")])]), n("b.py", "file", [n("f", "function")])]), n("top.py", "file", [n("g", "function")])]);
const ids = (open: string[], focus: string | null = null) => flatten(tree, new Set(open), focus).map((r) => r.id);

// the table of 06 §2 / 07 §1: open set × focus × a closed ancestor
const rows: { name: string; open: string[]; focus: string | null; expect: string[] }[] = [
  { name: "nothing open: the top level, closed", open: [], focus: null, expect: ["pkg", "top.py"] },
  { name: "pkg open", open: ["pkg"], focus: null, expect: ["pkg", "a.py", "b.py", "top.py"] },
  { name: "everything open", open: ["pkg", "a.py", "C", "b.py", "top.py"], focus: null, expect: ["pkg", "a.py", "C", "m", "b.py", "f", "top.py", "g"] },
  { name: "a node inside a closed one has no row, even when it is open", open: ["a.py", "C"], focus: null, expect: ["pkg", "top.py"] },
  { name: "a call id in the set has no row and does nothing", open: ["pkg", 'call:["f","g"]'], focus: null, expect: ["pkg", "a.py", "b.py", "top.py"] },
  { name: "a focus: the node alone at the top, always open, what is under it closed", open: [], focus: "pkg", expect: ["pkg", "a.py", "b.py"] },
  { name: "a focus on a file, with its class open", open: ["C"], focus: "a.py", expect: ["a.py", "C", "m"] },
  { name: "the focused node is open even when the set says nothing, and a node outside it is not drawn", open: ["top.py"], focus: "b.py", expect: ["b.py", "f"] },
  { name: "a focus on a leaf is one row", open: [], focus: "m", expect: ["m"] },
  { name: "a focus that the tree no longer has: the whole tree", open: [], focus: "gone", expect: ["pkg", "top.py"] },
];
for (const r of rows) test(r.name, () => assert.deepEqual(ids(r.open, r.focus), r.expect));

test("each row knows its depth, parent, whether it has children and is open, and its place among its siblings", () => {
  const all = flatten(tree, new Set(["pkg", "a.py", "C"]), null);
  const by = (id: string) => all.find((r) => r.id === id)!;
  assert.deepEqual([by("pkg").depth, by("a.py").depth, by("C").depth, by("m").depth], [0, 1, 2, 3]);
  assert.deepEqual([by("pkg").parent, by("a.py").parent, by("m").parent], [null, "pkg", "C"]);
  assert.deepEqual([by("pkg").hasChildren, by("pkg").open, by("b.py").hasChildren, by("b.py").open, by("m").hasChildren, by("m").open], [true, true, true, false, false, false]);
  assert.deepEqual([by("b.py").posInSet, by("b.py").setSize, by("top.py").posInSet, by("top.py").setSize, by("m").posInSet, by("m").setSize], [2, 2, 2, 2, 1, 1]);
});

test("a function inside a class is marked, so its icon is a method's", () => {
  const all = flatten(tree, new Set(["pkg", "a.py", "C"]), null);
  assert.deepEqual(all.filter((r) => r.inClass).map((r) => r.id), ["m"]);
  assert.equal(flatten(tree, new Set(["C"]), "C").find((r) => r.id === "m")!.inClass, true, "also under a focused class");
});

test("the focused row is marked, and is the only one", () => {
  const all = flatten(tree, new Set(), "pkg");
  assert.deepEqual(all.filter((r) => r.focusRoot).map((r) => r.id), ["pkg"]);
  assert.deepEqual([all[0].depth, all[0].parent, all[0].posInSet, all[0].setSize], [0, null, 1, 1]);
  assert.equal(flatten(tree, new Set(), null).some((r) => r.focusRoot), false);
});

test("a tree of 10,000 rows, everything open, flattens in under 50 ms", () => {
  const files = Array.from({ length: 100 }, (_, i) => n(`d${i}`, "folder", Array.from({ length: 99 }, (_, j) => n(`d${i}/f${j}.py`, "file"))));
  const big = n("/", "folder", files);
  const open = new Set(files.map((f) => f.id));
  flatten(big, open, null); // warm up
  const started = performance.now();
  const flat = flatten(big, open, null);
  const took = performance.now() - started;
  assert.equal(flat.length, 100 + 100 * 99);
  assert.ok(took < 50, `took ${took.toFixed(1)} ms`);
});
