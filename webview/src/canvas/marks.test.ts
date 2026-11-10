import assert from "node:assert/strict";
import { test } from "node:test";

import type { Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { holderOf, pathEdges } from "./marks.ts";

const n = (id: string, kind: Kind, children: TreeNode[] = []): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
// / ├ pkg ├ a.py ├ C ├ m      / ├ b.py
const tree = n("/", "folder", [n("pkg", "folder", [n("a.py", "file", [n("C", "class", [n("m", "function")])])]), n("b.py", "file")]);

test("a node that is not drawn is held by its nearest drawn ancestor", () => {
  assert.equal(holderOf(tree, new Set(["/", "pkg", "a.py", "b.py"]), "m"), "a.py");
  assert.equal(holderOf(tree, new Set(["/", "pkg", "b.py"]), "m"), "pkg");
  assert.equal(holderOf(tree, new Set(["/", "pkg", "a.py", "C", "m"]), "m"), null, "it is drawn");
  assert.equal(holderOf(tree, new Set(["/"]), null), null);
  assert.equal(holderOf(tree, new Set(["/"]), "gone"), null);
});

test("the edges to the selection run from the root of what is drawn", () => {
  assert.deepEqual([...pathEdges(tree, "/", "m")], ["/->pkg", "pkg->a.py", "a.py->C", "C->m"]);
  assert.deepEqual([...pathEdges(tree, "pkg", "m")], ["pkg->a.py", "a.py->C", "C->m"], "zoomed in on pkg");
  assert.equal(pathEdges(tree, "pkg", "b.py").size, 0, "the selection is outside the drawn root");
  assert.equal(pathEdges(tree, "/", null).size, 0);
});
