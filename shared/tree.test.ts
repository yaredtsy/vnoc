import assert from "node:assert/strict";
import { test } from "node:test";

import type { TreeNode } from "./messages";
import { allNodes, chainAt, docTarget, findNode, nodeAtPath, pathTo } from "./tree.ts";

const range = (from: number, to: number) => ({ start: { line: from, character: 0 }, end: { line: to, character: 0 } });
const node = (id: string, kind: TreeNode["kind"], children: TreeNode[] = [], more: Partial<TreeNode> = {}): TreeNode => ({
  id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children, ...more,
});

// pkg/a.py: class C (lines 2-9) with method m (4-6), and function top (11-12)
const m = node("m", "function", [], { path: "pkg/a.py", range: range(4, 6) });
const c = node("C", "class", [m], { path: "pkg/a.py", range: range(2, 9) });
const top = node("top", "function", [], { path: "pkg/a.py", range: range(11, 12) });
const file = node("pkg/a.py", "file", [c, top], { path: "pkg/a.py" });
const root = node("/", "folder", [node("pkg", "folder", [file], { path: "pkg" }), node("b.py", "file", [], { path: "b.py" })]);

const ids = (nodes: TreeNode[]) => nodes.map((n) => n.id);

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

test("a node is found by id, and a folder or file by its path", () => {
  assert.equal(findNode(root, "m")?.id, "m");
  assert.equal(findNode(root, "nope"), undefined);
  assert.equal(nodeAtPath(root, "pkg")?.id, "pkg");
  assert.equal(nodeAtPath(root, "pkg/a.py")?.id, "pkg/a.py");
  assert.equal(nodeAtPath(root, "")?.id, "/", "the opened folder itself");
  assert.equal(nodeAtPath(root, "pkg/other.py"), undefined);
});

test("pathTo goes from the root down to the node, and is null when the node is not there", () => {
  assert.deepEqual(ids(pathTo(root, "m")!), ["/", "pkg", "pkg/a.py", "C", "m"]);
  assert.deepEqual(ids(pathTo(root, "/")!), ["/"]);
  assert.equal(pathTo(root, "gone"), null);
});

test("allNodes lists every node once, parents before children", () => {
  assert.deepEqual(ids(allNodes(root)), ["/", "pkg", "pkg/a.py", "C", "m", "top", "b.py"]);
});
