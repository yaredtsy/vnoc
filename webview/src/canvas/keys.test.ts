import assert from "node:assert/strict";
import { test } from "node:test";

import type { Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { canvasKey } from "./keys.ts";
import { layout } from "./layout.ts";

const n = (id: string, kind: Kind, children: TreeNode[] = []): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
// / ├ a.py ├ f      / ├ b.py     (a.py is open, b.py has no children)
const root = n("/", "folder", [n("a.py", "file", [n("f", "function")]), n("b.py", "file")]);
const laid = layout(root, new Set(["a.py"])); // rows: / a.py f b.py
const closed = layout(root, new Set()); // rows: / a.py b.py

test("up and down move through the rows, and stop at the ends", () => {
  assert.deepEqual(canvasKey("ArrowDown", laid, root, "a.py"), { type: "select", id: "f" });
  assert.deepEqual(canvasKey("ArrowUp", laid, root, "b.py"), { type: "select", id: "f" });
  assert.deepEqual(canvasKey("ArrowUp", laid, root, "/"), { type: "select", id: "/" });
  assert.deepEqual(canvasKey("ArrowDown", laid, root, "b.py"), { type: "select", id: "b.py" });
  assert.deepEqual(canvasKey("ArrowDown", laid, root, null), { type: "select", id: "/" }, "nothing selected: the first row");
});

test("left selects the parent; right opens a closed card, else goes to the first child", () => {
  assert.deepEqual(canvasKey("ArrowLeft", laid, root, "f"), { type: "select", id: "a.py" });
  assert.equal(canvasKey("ArrowLeft", laid, root, "/"), null, "the root has no parent");
  assert.deepEqual(canvasKey("ArrowRight", closed, root, "a.py"), { type: "expand", ids: ["a.py"] });
  assert.deepEqual(canvasKey("ArrowRight", laid, root, "a.py"), { type: "select", id: "f" });
  assert.equal(canvasKey("ArrowRight", laid, root, "b.py"), null, "a leaf has nothing to open");
});

test("enter focuses, space opens and closes, escape clears the selection", () => {
  assert.deepEqual(canvasKey("Enter", laid, root, "a.py"), { type: "focus", id: "a.py" });
  assert.deepEqual(canvasKey(" ", laid, root, "a.py"), { type: "toggle", id: "a.py" });
  assert.equal(canvasKey(" ", laid, root, "b.py"), null);
  assert.deepEqual(canvasKey("Escape", laid, root, "a.py"), { type: "select", id: null });
  assert.equal(canvasKey("Escape", laid, root, null), null);
  assert.equal(canvasKey("x", laid, root, "a.py"), null);
});

// a function whose calls a probe counted has a chevron: the keys open it like any card with something under it
const withCalls = n("/", "folder", [n("a.py", "file", [n("f", "function"), n("g", "function")])]);
const probed = (open: string[]) => layout(withCalls, new Set(["a.py", ...open]), new Set(), new Map(), (key) => (key === "f" ? { state: "probed", count: 2 } : { state: "unknown" })); // rows: / a.py f g

test("→ opens a function that has calls, Space toggles it, and a function with none does nothing", () => {
  assert.deepEqual(canvasKey("ArrowRight", probed([]), withCalls, "f"), { type: "expand", ids: ["f"] });
  assert.deepEqual(canvasKey(" ", probed([]), withCalls, "f"), { type: "toggle", id: "f" });
  assert.equal(canvasKey("ArrowRight", probed([]), withCalls, "g"), null, "g has no calls to open");
  assert.equal(canvasKey(" ", probed([]), withCalls, "g"), null);
});

test("→ on an open function whose calls are not drawn as cards goes nowhere: the card below it is not its child", () => {
  assert.equal(canvasKey("ArrowRight", probed(["f"]), withCalls, "f"), null);
});
