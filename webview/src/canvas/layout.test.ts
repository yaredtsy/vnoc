import assert from "node:assert/strict";
import { test } from "node:test";

import { CALLS_HEAD_H, CARD_H, CALLS_MAX_ROWS, CALL_ROW_H, CODE_H, COL_W, ROW_H, callsHeight, canOpen, cardHeight, layout } from "./layout.ts";
import type { Calls } from "@vnoc/shared/calls.ts";
import type { Kind, TreeNode } from "@vnoc/shared/messages.ts";

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

test("a card with a Calls section is as tall as its rows, and what is below it starts lower", () => {
  const t = n("/", "folder", [n("a.py", "file", [n("f", "function"), n("g", "function")])]);
  const l = layout(t, new Set(["a.py"]), new Set(), new Map([["f", 3]]));
  const f = l.nodes.find((p) => p.id === "f")!;
  assert.equal(f.callRows, 3);
  assert.equal(cardHeight(f), CARD_H + CALLS_HEAD_H + 3 * CALL_ROW_H);
  assert.deepEqual(l.nodes.map((p) => p.y), [0, ROW_H, 2 * ROW_H, 3 * ROW_H + CALLS_HEAD_H + 3 * CALL_ROW_H]);
  assert.deepEqual(l.nodes.map((p) => p.callRows), [null, null, 3, null], "only the entry has a section");
});

test("the Calls section stops growing at ten rows (the rest scroll), and an empty one still has a line", () => {
  assert.equal(callsHeight(null), 0);
  assert.equal(callsHeight(0), CALLS_HEAD_H + CALL_ROW_H, "a line that says it calls nothing");
  assert.equal(callsHeight(CALLS_MAX_ROWS), callsHeight(CALLS_MAX_ROWS + 50));
  assert.equal(cardHeight({ codeOpen: true, callRows: 2 }), CARD_H + CODE_H + CALLS_HEAD_H + 2 * CALL_ROW_H, "open code and calls add up");
});

// ── a function or class card whose calls a probe (or its tree) counted (docs/16-ui-redesign/05-calls-as-nodes.md §3.1) ──────────────────────────────

const fn = (id: string): TreeNode => n(id, "function");
const project = n("/", "folder", [n("a.py", "file", [fn("f"), fn("g"), fn("h"), n("C", "class", [fn("m")])])]);
const calls = (table: Record<string, Calls>) => (key: string): Calls => table[key] ?? { state: "unknown" };
const card = (l: ReturnType<typeof layout>, id: string) => l.nodes.find((p) => p.id === id)!;

test("a function a probe counted: closed, a chevron, +N behind it, and it is not calls-nothing", () => {
  const l = layout(project, new Set(["a.py"]), new Set(), new Map(), calls({ f: { state: "probed", count: 3 } }));
  assert.deepEqual([card(l, "f").callsChevron, card(l, "f").collapsed, card(l, "f").plus, card(l, "f").callsNothing, card(l, "f").childCount], [true, true, 3, false, 0]);
});

test("a probe of 0: no chevron, nothing behind it, and the meta line says calls nothing", () => {
  const f = card(layout(project, new Set(["a.py"]), new Set(), new Map(), calls({ f: { state: "probed", count: 0 } })), "f");
  assert.deepEqual([f.callsChevron, f.collapsed, f.plus, f.callsNothing], [false, false, 0, true]);
});

test("a probe not answered yet: an empty slot, nothing said, and the card is as it was", () => {
  const f = card(layout(project, new Set(["a.py"])), "f");
  assert.deepEqual([f.callsChevron, f.collapsed, f.plus, f.callsNothing], [false, false, 0, false]);
});

test("open, a function with calls has its chevron turned and no +N", () => {
  const f = card(layout(project, new Set(["a.py", "f"]), new Set(), new Map(), calls({ f: { state: "probed", count: 3 } })), "f");
  assert.deepEqual([f.callsChevron, f.collapsed, f.plus], [true, false, 0]);
});

test("a function whose tree is known counts its children; one that is loading has a chevron and no count", () => {
  const known: Calls = { state: "known", children: [{ key: null, name: "x", qualname: "x", path: null, kind: "function", call_count: 1, call_sites: [], marks: [], children: [] }] };
  const l = layout(project, new Set(["a.py"]), new Set(), new Map(), calls({ f: known, g: { state: "loading" } }));
  assert.deepEqual([card(l, "f").callsChevron, card(l, "f").plus], [true, 1]);
  assert.deepEqual([card(l, "g").callsChevron, card(l, "g").plus, card(l, "g").collapsed], [true, 0, true]);
});

test("a class with methods keeps its structure children: its count is the methods, and a probe does not change it", () => {
  const c = card(layout(project, new Set(["a.py"]), new Set(), new Map(), calls({ C: { state: "probed", count: 9 } })), "C");
  assert.deepEqual([c.callsChevron, c.plus, c.collapsed, c.childCount], [false, 1, true, 1]);
});

test("the focused function is the root of the view: always open, never a chevron, but it can say it calls nothing", () => {
  const f = card(layout(fn("f"), new Set(), new Set(), new Map(), calls({ f: { state: "probed", count: 0 } })), "f");
  assert.deepEqual([f.collapsed, f.callsNothing], [false, true]);
  const g = card(layout(fn("g"), new Set(), new Set(), new Map(), calls({ g: { state: "probed", count: 4 } })), "g");
  assert.deepEqual([g.collapsed, g.callsChevron, g.plus], [false, true, 0]);
});

test("canOpen: structure, or calls", () => {
  assert.equal(canOpen({ childCount: 2, callsChevron: false }), true);
  assert.equal(canOpen({ childCount: 0, callsChevron: true }), true);
  assert.equal(canOpen({ childCount: 0, callsChevron: false }), false);
});
