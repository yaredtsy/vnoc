import assert from "node:assert/strict";
import { test } from "node:test";

import { chainAt, docTarget } from "./nodeAt.ts";
import type { TreeNode } from "./structureView";

const range = (from: number, to: number) => ({ start: { line: from, character: 0 }, end: { line: to, character: 0 } });
const node = (id: string, kind: TreeNode["kind"], children: TreeNode[] = [], more: Partial<TreeNode> = {}): TreeNode => ({ id, kind, name: id, path: id, range: null, syntax_error: false, docs: 0, link: null, children, ...more });

// pkg/a.py: class C (lines 2-9) with method m (4-6), and function top (11-12)
const m = node("m", "function", [], { path: "pkg/a.py", range: range(4, 6) });
const c = node("C", "class", [m], { path: "pkg/a.py", range: range(2, 9) });
const top = node("top", "function", [], { path: "pkg/a.py", range: range(11, 12) });
const file = node("pkg/a.py", "file", [c, top], { path: "pkg/a.py" });
const root = node("/", "folder", [node("pkg", "folder", [file], { path: "pkg" }), node("b.py", "file", [], { path: "b.py" })]);

const ids = (chain: TreeNode[]) => chain.map((n) => n.id);

test("the chain goes from the file to the innermost def around the line", () => {
  assert.deepEqual(ids(chainAt(root, "pkg/a.py", 5)), ["pkg/a.py", "C", "m"]);
  assert.deepEqual(ids(chainAt(root, "pkg/a.py", 8)), ["pkg/a.py", "C"], "in the class, below the method");
  assert.deepEqual(ids(chainAt(root, "pkg/a.py", 11)), ["pkg/a.py", "top"]);
});

test("a line outside every def is the file, and a file that is not in the tree is nothing", () => {
  assert.deepEqual(ids(chainAt(root, "pkg/a.py", 0)), ["pkg/a.py"]);
  assert.deepEqual(chainAt(root, "nowhere.py", 3), []);
});

test("the docs shown are the innermost node that has docs, else the innermost node", () => {
  const withDocs = (n: TreeNode, docs: number): TreeNode => ({ ...n, docs });
  assert.equal(docTarget([withDocs(file, 1), withDocs(c, 2), m])?.id, "C", "m has none, so its class");
  assert.equal(docTarget([withDocs(file, 1), c, m])?.id, "pkg/a.py", "then its file");
  assert.equal(docTarget([file, c, m])?.id, "m", "none anywhere: the innermost, to write the first doc");
  assert.equal(docTarget([]), undefined);
});
