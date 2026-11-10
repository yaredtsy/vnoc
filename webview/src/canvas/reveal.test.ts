import assert from "node:assert/strict";
import { test } from "node:test";

import type { Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { reveal } from "./reveal.ts";

const n = (id: string, kind: Kind, children: TreeNode[] = []): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });

// / ├ pkg ├ a.py ├ C ├ m      and   / ├ b.py ├ f
const tree = n("/", "folder", [n("pkg", "folder", [n("a.py", "file", [n("C", "class", [n("m", "function")])])]), n("b.py", "file", [n("f", "function")])]);

test("the ancestors of the node are opened, the node itself is not", () => {
  assert.deepEqual(reveal(tree, [], "m"), { focus: [], open: ["/", "pkg", "a.py", "C"] });
});

test("a node inside the focused one keeps the focus", () => {
  assert.deepEqual(reveal(tree, ["pkg"], "m")?.focus, ["pkg"]);
});

test("a node outside the focus goes back to the nearest entry that contains it, or to nothing", () => {
  assert.deepEqual(reveal(tree, ["pkg", "a.py"], "f")?.focus, [], "nothing on the stack contains f");
  assert.deepEqual(reveal(tree, ["pkg", "b.py"], "C")?.focus, ["pkg"], "b.py does not contain C, pkg does");
});

test("a node that is not in the tree is not revealed", () => {
  assert.equal(reveal(tree, [], "gone"), null);
});
