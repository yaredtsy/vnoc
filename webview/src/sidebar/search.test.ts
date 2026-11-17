import assert from "node:assert/strict";
import { test } from "node:test";

import type { Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { MAIN, emptySession, reduce } from "@vnoc/shared/session.ts";
import { nodesHeader, pickIntent, searchKey, splitMatch } from "./search.ts";

const n = (id: string, kind: Kind, path: string, name: string, children: TreeNode[] = []): TreeNode => ({ id, kind, name, path, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
const file = (path: string, ...fns: string[]) => n(path, "file", path, path.split("/").pop()!, fns.map((f) => n(`${path}::${f}`, "function", path, f)));

// the tree the reveal test uses:
// /  ├ billing/ ├ invoice.py ├ total   ├ bill_all      ├ Invoice (class) ├ pay
//    │          └ rebilling.py ├ rebill
//    └ shop/cart.py ├ add_item  └ billing_note
const tree = n("/", "folder", "", "shop", [
  n("billing", "folder", "billing", "billing", [
    n("billing/invoice.py", "file", "billing/invoice.py", "invoice.py", [
      n("billing/invoice.py::total", "function", "billing/invoice.py", "total"),
      n("billing/invoice.py::bill_all", "function", "billing/invoice.py", "bill_all"),
      n("billing/invoice.py::Invoice", "class", "billing/invoice.py", "Invoice", [n("billing/invoice.py::Invoice.pay", "function", "billing/invoice.py", "pay")]),
    ]),
    file("billing/rebilling.py", "rebill"),
  ]),
  n("shop", "folder", "shop", "shop", [file("shop/cart.py", "add_item", "billing_note")]),
]);
// a name cut around the ranges the core found (UTF-16 offsets, as `slice` takes them)
const pieces = (name: string, ranges: [number, number][]) => splitMatch(name, ranges).map((p) => (p.bold ? `[${p.text}]` : p.text));
const splits: { name: string; text: string; ranges: [number, number][]; expect: string[] }[] = [
  { name: "no range: the whole name, plain", text: "checkout_order", ranges: [], expect: ["checkout_order"] },
  { name: "one range in the middle", text: "rebilling.py", ranges: [[2, 6]], expect: ["re", "[bill]", "ing.py"] },
  { name: "two ranges (tier 2: `chor`)", text: "checkout_order", ranges: [[0, 2], [9, 11]], expect: ["[ch]", "eckout_", "[or]", "der"] },
  { name: "a range at the start", text: "bill_all", ranges: [[0, 4]], expect: ["[bill]", "_all"] },
  { name: "a range at the end", text: "bill_all", ranges: [[5, 8]], expect: ["bill_", "[all]"] },
  { name: "the whole name", text: "total", ranges: [[0, 5]], expect: ["[total]"] },
  { name: "two ranges that touch: two bold pieces side by side, no empty piece between", text: "abcd", ranges: [[0, 2], [2, 4]], expect: ["[ab]", "[cd]"] },
  { name: "`é` before a range: UTF-16 offsets, one unit for `é` (the core's `café_order` with `or`)", text: "café_order", ranges: [[5, 7]], expect: ["café_", "[or]", "der"] },
  { name: "an emoji is two units: the range after it is counted in units", text: "😀_order", ranges: [[3, 5]], expect: ["😀_", "[or]", "der"] },
  { name: "an empty name has no pieces", text: "", ranges: [], expect: [] },
];
for (const t of splits) test(`splitMatch: ${t.name}`, () => assert.deepEqual(pieces(t.text, t.ranges), t.expect));

test("the header says how many rows, and of how many when the limit cut them", () => {
  assert.equal(nodesHeader(12, 12), "Nodes 12");
  assert.equal(nodesHeader(200, 1340), "Nodes 200 of 1,340");
  assert.equal(nodesHeader(1, 2), "Nodes 1 of 2");
});

// the box's keys
const keys: { name: string; key: string; count: number; active: number; expect: ReturnType<typeof searchKey> }[] = [
  { name: "↓ moves to the next result", key: "ArrowDown", count: 5, active: 0, expect: { type: "move", to: 1 } },
  { name: "↓ stops at the last", key: "ArrowDown", count: 5, active: 4, expect: { type: "move", to: 4 } },
  { name: "↑ moves to the previous", key: "ArrowUp", count: 5, active: 3, expect: { type: "move", to: 2 } },
  { name: "↑ stops at the first", key: "ArrowUp", count: 5, active: 0, expect: { type: "move", to: 0 } },
  { name: "Enter picks the active result", key: "Enter", count: 5, active: 2, expect: { type: "pick" } },
  { name: "Enter with no results does nothing", key: "Enter", count: 0, active: 0, expect: null },
  { name: "↓ with no results does nothing", key: "ArrowDown", count: 0, active: 0, expect: null },
  { name: "Esc clears the box, results or not", key: "Escape", count: 0, active: 0, expect: { type: "clear" } },
  { name: "a letter is the box's own", key: "a", count: 5, active: 0, expect: null },
  { name: "Home is the box's own (the caret)", key: "Home", count: 5, active: 2, expect: null },
];
for (const k of keys) test(`search key: ${k.name}`, () => assert.deepEqual(searchKey(k.key, k.count, k.active), k.expect));

test("Enter on a result the focus hides: the focus shortens until it holds the node, the ancestors open, the node is selected (03 §3)", () => {
  const lookup = () => undefined;
  let s = reduce(emptySession(), MAIN, { type: "focus", id: "shop/cart.py" }, tree, lookup);
  assert.deepEqual([...s.contexts[MAIN].focus], ["shop", "shop/cart.py"]);
  // a result under billing/, which the focus hides
  s = reduce(s, MAIN, pickIntent("billing/invoice.py::Invoice.pay"), tree, lookup);
  const c = s.contexts[MAIN];
  assert.deepEqual([...c.focus], [], "no step of the focus holds it: the whole project");
  assert.equal(c.selected, "billing/invoice.py::Invoice.pay");
  assert.deepEqual([...(c.open[""] ?? [])].sort(), ["billing", "billing/invoice.py", "billing/invoice.py::Invoice"].sort(), "its ancestors are open");
  // a result inside the focus keeps it
  let inside = reduce(emptySession(), MAIN, { type: "focus", id: "billing" }, tree, lookup);
  inside = reduce(inside, MAIN, pickIntent("billing/invoice.py::total"), tree, lookup);
  assert.deepEqual([...inside.contexts[MAIN].focus], ["billing"], "a node the focus holds does not shorten it");
  assert.equal(inside.contexts[MAIN].selected, "billing/invoice.py::total");
  // a result the focus partly holds: shortened only as far as needed
  let partly = reduce(emptySession(), MAIN, { type: "focus", id: "billing/invoice.py" }, tree, lookup);
  partly = reduce(partly, MAIN, pickIntent("billing/rebilling.py::rebill"), tree, lookup);
  assert.deepEqual([...partly.contexts[MAIN].focus], ["billing"], "back to the step that holds it");
});
