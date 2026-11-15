import assert from "node:assert/strict";
import { test } from "node:test";

import type { Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { MAIN, emptySession, reduce } from "@vnoc/shared/session.ts";
import { RESULT_LIMIT, indexNodes, matchRange, pickIntent, rankNodes, searchKey, splitMatch } from "./search.ts";

const n = (id: string, kind: Kind, path: string, name: string, children: TreeNode[] = []): TreeNode => ({ id, kind, name, path, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
const file = (path: string, ...fns: string[]) => n(path, "file", path, path.split("/").pop()!, fns.map((f) => n(`${path}::${f}`, "function", path, f)));

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
const index = indexNodes(tree);
const names = (q: string, limit?: number) => rankNodes(index, q, limit).map((r) => r.node.name);

test("every node is searched in reading order, but the opened folder itself", () => {
  assert.deepEqual(index.map((e) => e.node.name), ["billing", "invoice.py", "total", "bill_all", "Invoice", "pay", "rebilling.py", "rebill", "shop", "cart.py", "add_item", "billing_note"]);
  assert.deepEqual(index.filter((e) => e.inClass).map((e) => e.node.name), ["pay"], "a function inside a class is a method");
});

// the ranking of 06 §5: name before path, prefix before substring; within a tier the shorter name first, then reading order
const ranking: { name: string; query: string; expect: [name: string, tier: number][] }[] = [
  {
    name: "`bill`: name prefixes, then name substrings, then what holds it only in its path (billing/…)",
    query: "bill",
    expect: [["billing", 0], ["bill_all", 0], ["billing_note", 0], ["rebill", 1], ["rebilling.py", 1], ["pay", 2], ["total", 2], ["Invoice", 2], ["invoice.py", 2]],
  },
  { name: "case does not matter, and the whole name is the shortest prefix match", query: "INVOICE", expect: [["Invoice", 0], ["invoice.py", 0], ["pay", 3], ["total", 3], ["bill_all", 3]] },
  { name: "`billing/` finds by path: everything under that folder (not the folder itself), the shorter names first", query: "billing/", expect: [["pay", 2], ["total", 2], ["rebill", 2], ["Invoice", 2], ["bill_all", 2], ["invoice.py", 2], ["rebilling.py", 2]] },
  { name: "a path substring in the middle of the path", query: "illing/inv", expect: [["pay", 3], ["total", 3], ["Invoice", 3], ["bill_all", 3], ["invoice.py", 3]] },
  { name: "an empty or blank query finds nothing", query: "  ", expect: [] },
  { name: "nothing matches", query: "zzz", expect: [] },
];
for (const r of ranking) test(`ranking: ${r.name}`, () => assert.deepEqual(rankNodes(index, r.query).map((x) => [x.node.name, x.tier]), r.expect));

test("ranking: the four tiers, in order: name prefix, name substring, path prefix, path substring", () => {
  const four = n("/", "folder", "", "p", [
    n("lib/shopping.py::a", "function", "lib/shopping.py", "x1"), // path holds `shop`, not at its start
    n("shop/b.py::b", "function", "shop/b.py", "x2"), // path starts with it
    n("c.py::myshop", "function", "c.py", "myshop"), // name holds it
    n("d.py::shopper", "function", "d.py", "shopper"), // name starts with it
  ]);
  assert.deepEqual(rankNodes(indexNodes(four), "shop").map((r) => [r.node.name, r.tier]), [["shopper", 0], ["myshop", 1], ["x2", 2], ["x1", 3]]);
});

test("ranking: a name prefix beats a path match, whatever the length", () => {
  const rows = rankNodes(index, "shop");
  assert.deepEqual([rows[0].node.name, rows[0].tier], ["shop", 0]);
  assert.ok(rows.slice(1).every((r) => r.tier === 2), "what is under shop/ matches by path");
});

test("the 200 cap: more matches than that show the best 200", () => {
  const many = n("/", "folder", "", "p", [
    n("x/z.py", "file", "x/z.py", "z.py", Array.from({ length: 300 }, (_, i) => n(`x/z.py::f${i}`, "function", "x/z.py", `f${i}`))),
    n("exact", "function", "x/z.py", "f"), // a whole-name match: first
  ]);
  const all = indexNodes(many);
  const rows = rankNodes(all, "f");
  assert.equal(RESULT_LIMIT, 200);
  assert.equal(rows.length, 200);
  assert.equal(rows[0].node.name, "f", "the best first, whatever the order in the tree");
  assert.deepEqual(rankNodes(all, "f", 5).map((r) => r.node.name), ["f", "f0", "f1", "f2", "f3"], "the shorter names first, then reading order");
});

test("the letters to draw bold: the match in the name, none when only the path matched", () => {
  assert.deepEqual(matchRange("bill_all", "BILL"), [0, 4]);
  assert.deepEqual(matchRange("rebilling.py", "bill"), [2, 6]);
  assert.equal(matchRange("total", "bill"), null);
  assert.deepEqual(rankNodes(index, "bill").map((r) => r.match?.join("-") ?? null), ["0-4", "0-4", "0-4", "2-6", "2-6", null, null, null, null], "bold for a name match, nothing for a path match");
  assert.equal(rankNodes(index, "billing/")[0].match, null, "a path match has nothing bold in the name");
  assert.deepEqual(splitMatch("rebilling.py", [2, 6]), ["re", "bill", "ing.py"]);
  assert.deepEqual(splitMatch("total", null), ["total", "", ""]);
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
