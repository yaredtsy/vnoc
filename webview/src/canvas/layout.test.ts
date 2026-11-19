import assert from "node:assert/strict";
import { test } from "node:test";

import { type Calls, callId } from "@vnoc/shared/calls.ts";
import type { CallMark, CallSite, CallTreeNode, Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { CARD_H, CODE_H, COL_W, FOOTER_H, HEADER_H, LINE_H, MAX_CARDS, META_H, ROW_H, SITE_LIST_PAD, SITE_ROW_H, canOpen, cardHeight, layout, siteListHeight } from "./layout.ts";

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

// ── a function or class card whose calls a probe (or its tree) counted (docs/16-ui-redesign/05-calls-as-nodes.md §3.1) ──────────────────────────────

const fn = (id: string): TreeNode => n(id, "function");
const project = n("/", "folder", [n("a.py", "file", [fn("f"), fn("g"), fn("h"), n("C", "class", [fn("m")])])]);
const calls = (table: Record<string, Calls>) => (key: string): Calls => table[key] ?? { state: "unknown" };
const card = (l: ReturnType<typeof layout>, id: string) => l.nodes.find((p) => p.id === id)!;

test("a function a probe counted: closed, a chevron, +N behind it, and the footer has no number: the +N says it", () => {
  const l = layout(project, new Set(["a.py"]), new Set(), calls({ f: { state: "probed", count: 3 } }));
  assert.deepEqual([card(l, "f").callsChevron, card(l, "f").collapsed, card(l, "f").plus, card(l, "f").callsFooter, card(l, "f").childCount], [true, true, 3, null, 0]);
});

test("a probe of 0: no chevron, nothing behind it, and the footer says calls nothing", () => {
  const f = card(layout(project, new Set(["a.py"]), new Set(), calls({ f: { state: "probed", count: 0 } })), "f");
  assert.deepEqual([f.callsChevron, f.collapsed, f.plus, f.callsFooter], [false, false, 0, { state: "nothing" }]);
});

test("a probe not answered yet: an empty slot, nothing said, and the card is as it was", () => {
  const f = card(layout(project, new Set(["a.py"])), "f");
  assert.deepEqual([f.callsChevron, f.collapsed, f.plus, f.callsFooter], [false, false, 0, null]);
});

test("open, a function with calls has its chevron turned and no +N", () => {
  const f = card(layout(project, new Set(["a.py", "f"]), new Set(), calls({ f: { state: "probed", count: 3 } })), "f");
  assert.deepEqual([f.callsChevron, f.collapsed, f.plus], [true, false, 0]);
});

test("a function whose tree is known counts its children; one that is loading has a chevron and no count", () => {
  const known: Calls = { state: "known", children: [{ key: null, name: "x", qualname: "x", path: null, kind: "function", call_count: 1, call_sites: [], marks: [], children: [] }] };
  const l = layout(project, new Set(["a.py"]), new Set(), calls({ f: known, g: { state: "loading" } }));
  assert.deepEqual([card(l, "f").callsChevron, card(l, "f").plus], [true, 1]);
  assert.deepEqual([card(l, "g").callsChevron, card(l, "g").plus, card(l, "g").collapsed], [true, 0, true]);
});

test("a class with methods and no __init__ of its own owns calls too: closed, it hides its methods and its calls", () => {
  const c = card(layout(project, new Set(["a.py"]), new Set(), calls({ C: { state: "probed", count: 9 } })), "C");
  assert.deepEqual([c.callsChevron, c.plus, c.collapsed, c.childCount], [true, 10, true, 1]);
  const bare = card(layout(project, new Set(["a.py"])), "C");
  assert.deepEqual([bare.callsChevron, bare.plus, bare.collapsed], [false, 1, true], "nothing known about its calls: its methods only");
});

test("the focused function is the root of the view: always open, never a chevron, but it can say it calls nothing", () => {
  const f = card(layout(fn("f"), new Set(), new Set(), calls({ f: { state: "probed", count: 0 } })), "f");
  assert.deepEqual([f.collapsed, f.callsFooter], [false, { state: "nothing" }]);
  const g = card(layout(fn("g"), new Set(), new Set(), calls({ g: { state: "probed", count: 4 } })), "g");
  assert.deepEqual([g.collapsed, g.callsChevron, g.plus], [false, true, 0]);
});

test("canOpen: structure, or calls", () => {
  assert.equal(canOpen({ childCount: 2, callsChevron: false }), true);
  assert.equal(canOpen({ childCount: 0, callsChevron: true }), true);
  assert.equal(canOpen({ childCount: 0, callsChevron: false }), false);
});

// ── call cards (docs/16-ui-redesign/03-canvas.md §1, §6; 04-node-cards.md §2, §6) ───────────────────────────────────────────────────────────────────

const site = (line: number, path: string | null = "app.py"): CallSite => ({ start: { line: line - 1, character: 4 }, end: { line: line - 1, character: 9 }, path });
const call = (key: string | null, name: string, over: Partial<CallTreeNode> = {}, children: CallTreeNode[] = []): CallTreeNode => ({
  key, name, qualname: name, path: key === null ? null : "app.py", kind: "function", call_count: 1, call_sites: [site(5)], marks: [], children, ...over,
});

// / ├ app.py ├ checkout (calls order and lib; has a nested def `helper`)      order calls reserve, check
//           ├ order ├ reserve ├ check      └ K (a class with method run)
const range = (from: number) => ({ start: { line: from, character: 0 }, end: { line: from + 3, character: 0 } });
const withRange = (node: TreeNode, from: number): TreeNode => ({ ...node, path: "app.py", range: range(from) }); // a def lives in its file
const app = n("/", "folder", [
  n("app.py", "file", [
    withRange({ ...n("checkout", "function", [withRange(n("helper", "function"), 3)]), docs: 2 }, 1),
    withRange(n("order", "function"), 10),
    withRange(n("reserve", "function"), 20),
    withRange(n("check", "function"), 30),
    n("K", "class", [withRange(n("K.run", "function"), 40)]),
  ]),
]);

const orderCall = call("order", "order", { call_count: 2, call_sites: [site(16), site(31)] }, [call("reserve", "reserve"), call("check", "check", { call_sites: [site(7), site(8), site(9), site(10)] })]);
const checkoutTree: Calls = { state: "known", children: [orderCall, call(null, "print_log", { call_sites: [site(18)] })] };
const known = (children: CallTreeNode[]): Calls => ({ state: "known", children });
const Corder = callId("checkout", ["order"]);
const Cprint = callId("checkout", ["?::print_log"]);
const callsTable = (table: Record<string, Calls>) => (id: string): Calls => table[id] ?? { state: "unknown" };
const ids = (l: ReturnType<typeof layout>) => l.nodes.map((p) => p.id);

test("a function open with its calls known: its nested defs first, then its calls as cards, then what follows it", () => {
  const l = layout(app, new Set(["app.py", "checkout"]), new Set(), callsTable({ checkout: checkoutTree }));
  assert.deepEqual(ids(l), ["/", "app.py", "checkout", "helper", Corder, Cprint, "order", "reserve", "check", "K"]);
  assert.deepEqual(l.nodes.filter((p) => p.role === "call").map((p) => [p.name, p.key, p.depth]), [["order", "order", 3], ["print_log", null, 3]]);
});

test("the edges of a call are marked `call` (dashed) and the structure's are not", () => {
  const l = layout(app, new Set(["app.py", "checkout"]), new Set(), callsTable({ checkout: checkoutTree }));
  const kind = (target: string) => l.edges.find((e) => e.target === target)?.kind;
  assert.deepEqual([kind("helper"), kind(Corder), kind(Cprint), kind("checkout")], ["node", "call", "call", "node"]);
  assert.equal(l.edges.find((e) => e.target === Corder)?.source, "checkout");
});

test("a call card is a card of the callee: its kind, its range and docs from the structure, its place and its sites from the call", () => {
  const l = layout(app, new Set(["app.py", "checkout"]), new Set(), callsTable({ checkout: checkoutTree }));
  const order = l.nodes.find((p) => p.id === Corder)!;
  assert.deepEqual([order.role, order.kind, order.key, order.path, order.line, order.parentId], ["call", "function", "order", "app.py", 11, "checkout"]);
  assert.deepEqual(order.range, range(10), "the callee's code is what `</>` shows");
  assert.deepEqual([order.call!.count, order.call!.lines, order.call!.callerName, order.call!.callerPath], [2, [16, 31], "checkout", "app.py"]);
  const print = l.nodes.find((p) => p.id === Cprint)!;
  assert.deepEqual([print.key, print.range, print.line, print.call!.look.unknown], [null, null, null, true], "a callee the project does not know: no code, no line");
});

test("focused on a function, a call card still finds its callee in the whole project: range, docs, link and inClass (the callee is outside the focus)", () => {
  const link = { state: "linked" } as unknown as TreeNode["link"];
  const run = { ...withRange(n("K.run", "function"), 40), docs: 3, link };
  const project = n("/", "folder", [
    n("app.py", "file", [withRange(n("checkout", "function"), 1)]),
    n("lib.py", "file", [n("K", "class", [run])]),
  ]);
  const focused = project.children[0].children[0]; // checkout: lib.py and K are not in what is drawn
  const tree: Calls = { state: "known", children: [call("K.run", "run", { path: "lib.py" }), call("nope", "nope")] };
  const l = layout(focused, new Set(), new Set(), callsTable({ checkout: tree }), new Set(), project);
  const found = l.nodes.find((p) => p.id === callId("checkout", ["K.run"]))!;
  assert.deepEqual(found.range, range(40));
  assert.deepEqual([found.docs, found.link, found.inClass, found.line], [3, link, true, 41]);
  // a key that is in neither tree has none of it
  const lost = l.nodes.find((p) => p.id === callId("checkout", ["nope"]))!;
  assert.deepEqual([lost.range, lost.docs, lost.link, lost.inClass], [null, 0, null, false]);
  // without the whole tree the index is the drawn root's, as before
  const alone = layout(focused, new Set(), new Set(), callsTable({ checkout: tree })).nodes.find((p) => p.id === callId("checkout", ["K.run"]))!;
  assert.deepEqual([alone.range, alone.docs, alone.link, alone.inClass], [null, 0, null, false]);
  assert.deepEqual(l.nodes.map((p) => p.id), ["checkout", callId("checkout", ["K.run"]), callId("checkout", ["nope"])], "what is drawn is the focused root and its calls, not the whole project");
});

test("opening a call card draws what its callee calls, one level deeper, with its own call id", () => {
  const table = { checkout: checkoutTree, [Corder]: known(orderCall.children) };
  const l = layout(app, new Set(["app.py", "checkout", Corder]), new Set(), callsTable(table));
  assert.deepEqual(ids(l).slice(4, 8), [Corder, callId("checkout", ["order", "reserve"]), callId("checkout", ["order", "check"]), Cprint]);
  assert.equal(l.nodes.find((p) => p.id === callId("checkout", ["order", "reserve"]))!.depth, 4);
  assert.equal(l.edges.find((e) => e.target === callId("checkout", ["order", "reserve"]))!.source, Corder);
});

test("a call card closed says how many cards it hides; calls not known yet: a chevron and no +N only for a cut node", () => {
  const closed = layout(app, new Set(["app.py", "checkout"]), new Set(), callsTable({ checkout: checkoutTree, [Corder]: known(orderCall.children) }));
  const order = closed.nodes.find((p) => p.id === Corder)!;
  assert.deepEqual([order.callsChevron, order.collapsed, order.plus], [true, true, 2]);
  const noInfo = layout(app, new Set(["app.py", "checkout"]), new Set(), callsTable({ checkout: { state: "known", children: [call("order", "order")] } }));
  const bare = noInfo.nodes.find((p) => p.id === Corder)!;
  assert.deepEqual([bare.callsChevron, bare.plus], [false, 0], "a call with nothing known under it has no chevron");
  const cut = layout(app, new Set(["app.py", "checkout"]), new Set(), callsTable({ checkout: { state: "known", children: [call("order", "order", { marks: ["cut"] })] } }));
  const cutCard = cut.nodes.find((p) => p.id === Corder)!;
  assert.deepEqual([cutCard.callsChevron, cutCard.collapsed, cutCard.plus], [true, true, 0], "cut: the walk stopped, opening asks for the callee's tree");
});

test("a recursive call is shown and not entered: no chevron, even when it is in the open set", () => {
  const recursive: CallMark[] = ["recursive"];
  const l = layout(app, new Set(["app.py", "checkout", Corder]), new Set(), callsTable({ checkout: { state: "known", children: [call("order", "order", { marks: recursive }, [call("reserve", "reserve")])] } }));
  const order = l.nodes.find((p) => p.id === Corder)!;
  assert.deepEqual([order.callsChevron, order.collapsed], [false, false]);
  assert.equal(l.nodes.length, ids(l).length);
  assert.ok(!ids(l).includes(callId("checkout", ["order", "reserve"])), "what is under it is not drawn");
});

test("a cut node that is open draws its callee's own tree: the ids below belong to the callee", () => {
  const cutOrder = call("order", "order", { marks: ["cut"] });
  const table = { checkout: { state: "known", children: [cutOrder] } as Calls, [Corder]: known([call("reserve", "reserve")]) };
  const l = layout(app, new Set(["app.py", "checkout", Corder]), new Set(), callsTable(table));
  assert.ok(ids(l).includes(callId("order", ["reserve"])), `owner order, not checkout: ${ids(l)}`);
});

test("a class with no __init__ of its own: its methods first, then its calls", () => {
  const table = { K: known([call("order", "order")]) };
  const l = layout(app, new Set(["app.py", "K"]), new Set(), callsTable(table));
  assert.deepEqual(ids(l).slice(-3), ["K", "K.run", callId("K", ["order"])]);
});

test("a function inside a class is a method: its card says so, and so does a call card for it", () => {
  const l = layout(app, new Set(["app.py", "K", "checkout"]), new Set(), callsTable({ checkout: { state: "known", children: [call("K.run", "run")] } }));
  assert.equal(l.nodes.find((p) => p.id === "K.run")!.inClass, true);
  assert.equal(l.nodes.find((p) => p.id === "checkout")!.inClass, false);
  assert.equal(l.nodes.find((p) => p.id === callId("checkout", ["K.run"]))!.inClass, true, "the callee's parent is a class");
});

test("heights: every card is 80 px (32 + 24 + 1 + 23); +220 with its code; a call card's open site list adds a line for each site", () => {
  assert.equal(cardHeight({ codeOpen: false, siteRows: 0 }), CARD_H);
  assert.equal(CARD_H, 80);
  assert.equal(HEADER_H + META_H + LINE_H + FOOTER_H, CARD_H, "the four rows are the card");
  assert.deepEqual([HEADER_H, META_H, LINE_H, FOOTER_H], [32, 24, 1, 23]);
  assert.equal(cardHeight({ codeOpen: true, siteRows: 0 }), CARD_H + CODE_H);
  assert.equal(siteListHeight(4), 4 * SITE_ROW_H + SITE_LIST_PAD);
  assert.equal(siteListHeight(0), 0);
  assert.equal(cardHeight({ codeOpen: true, siteRows: 4 }), CARD_H + CODE_H + siteListHeight(4));
});

test("a call card with more than two sites shows its list when it is in the open set, and what is below it starts lower", () => {
  const withSites = call("order", "order", { call_sites: [site(1), site(2), site(3), site(4)] });
  const table = callsTable({ checkout: { state: "known", children: [withSites, call("check", "check")] } });
  const shut = layout(app, new Set(["app.py", "checkout"]), new Set(), table);
  const open = layout(app, new Set(["app.py", "checkout"]), new Set(), table, new Set([Corder]));
  const [a, b] = [callId("checkout", ["order"]), callId("checkout", ["check"])];
  assert.equal(open.nodes.find((p) => p.id === a)!.siteRows, 4);
  assert.equal(shut.nodes.find((p) => p.id === a)!.siteRows, 0);
  assert.equal(open.nodes.find((p) => p.id === b)!.y - shut.nodes.find((p) => p.id === b)!.y, siteListHeight(4));
  // two sites are both written on the meta line: there is no list to open
  const two = layout(app, new Set(["app.py", "checkout"]), new Set(), callsTable({ checkout: { state: "known", children: [call("order", "order", { call_sites: [site(1), site(2)] })] } }), new Set([Corder]));
  assert.equal(two.nodes.find((p) => p.id === Corder)!.siteRows, 0);
});

test("the callee's code in a call card pushes the rows below it down, as any card's code does", () => {
  const table = callsTable({ checkout: { state: "known", children: [call("order", "order"), call("check", "check")] } });
  const shut = layout(app, new Set(["app.py", "checkout"]), new Set(), table);
  const open = layout(app, new Set(["app.py", "checkout"]), new Set([Corder]), table);
  const check = callId("checkout", ["check"]);
  assert.equal(open.nodes.find((p) => p.id === check)!.y - shut.nodes.find((p) => p.id === check)!.y, CODE_H);
});

test("hidden counts structure nodes only: call cards are not part of the tree that is closed", () => {
  const l = layout(app, new Set(["app.py", "checkout"]), new Set(), callsTable({ checkout: checkoutTree }));
  // nine structure nodes; K is closed, so K.run is not drawn; the call cards are extra rows, not part of the nine
  assert.equal(l.hidden, 1);
  assert.equal(l.nodes.filter((p) => p.role === "node").length, 8);
});

test("two calls to one callee from one parent are one card (the merge), and the same callee under two parents is two cards with their own ids", () => {
  const l = layout(app, new Set(["app.py", "checkout", Corder]), new Set(), callsTable({ checkout: { state: "known", children: [call("order", "order", { call_count: 2 }, [call("check", "check")]), call("check", "check")] }, [Corder]: known([call("check", "check")]) }));
  const checks = l.nodes.filter((p) => p.key === "check" && p.role === "call");
  assert.equal(checks.length, 2);
  assert.notEqual(checks[0].id, checks[1].id);
  assert.equal(new Set(ids(l)).size, ids(l).length, "every id is unique, so a toggle always hits one card");
});

// ── the footer's calls state, and what a folder or file holds (04 §1) ───────────────────────────────────────────────

test("the footer's calls state: a spinner while fetching, a failure with its reason; a count is the +N, not a footer chip", () => {
  const l = layout(project, new Set(["a.py"]), new Set(), calls({ f: { state: "loading" }, g: { state: "failed", message: "boom" }, h: { state: "probed", count: 2 } }));
  assert.deepEqual([card(l, "f").callsFooter, card(l, "g").callsFooter, card(l, "h").callsFooter], [{ state: "loading" }, { state: "failed", message: "boom" }, null]);
  const open = layout(project, new Set(["a.py", "h"]), new Set(), calls({ h: { state: "probed", count: 2 } }));
  assert.equal(card(open, "h").callsFooter, null, "open: the calls are the cards");
});

test("a class that has methods does not say it calls nothing: its methods are what is under it", () => {
  const c = card(layout(project, new Set(["a.py"]), new Set(), calls({ C: { state: "probed", count: 0 } })), "C");
  assert.equal(c.callsFooter, null);
});

test("a file and a folder know what they hold directly, by kind; a function holds nothing", () => {
  const l = layout(project, new Set(["a.py"]));
  assert.deepEqual(card(l, "/").contents, { folders: 0, files: 1, classes: 0, functions: 0 });
  assert.deepEqual(card(l, "a.py").contents, { folders: 0, files: 0, classes: 1, functions: 3 });
  assert.deepEqual(card(l, "f").contents, { folders: 0, files: 0, classes: 0, functions: 0 });
});

test("a call card's footer says what its callee calls, except a recursive call or a callee the project does not know", () => {
  const open = new Set(["app.py", "checkout"]);
  const footerOf = (id: string, children: CallTreeNode[], table: Record<string, Calls>) => layout(app, open, new Set(), callsTable({ checkout: { state: "known", children }, ...table })).nodes.find((p) => p.id === id)!.callsFooter;
  assert.equal(footerOf(Corder, [call("order", "order")], { [Corder]: known(orderCall.children) }), null, "closed, its tree known: the +N says it");
  assert.deepEqual(footerOf(Corder, [call("order", "order")], { [Corder]: known([]) }), { state: "nothing" });
  assert.equal(footerOf(Corder, [call("order", "order", { marks: ["recursive"] })], { [Corder]: known([]) }), null, "it calls itself: not nothing");
  assert.equal(footerOf(Corder, [call("order", "order")], {}), null, "nothing known");
  assert.deepEqual(footerOf(Corder, [call("order", "order", { marks: ["cut"] })], { [Corder]: { state: "failed", message: "x" } }), { state: "failed", message: "x" }, "a cut call whose callee's tree failed: Retry");
});

// ── 8.4b / 03 §6.1: a drawing never holds more than MAX_CARDS; an open card whose family does not fit is drawn closed ──

const files = (count: number, prefix = "f") => Array.from({ length: count }, (_, i) => n(`${prefix}${i}`, "file"));
const folder = (id: string, count: number) => n(id, "folder", files(count, `${id}/`));

test("the budget: the root and its whole family are always drawn, past 200 if they must be, and nothing is held back, so no note", () => {
  for (const [count, drawn] of [[198, 199], [199, 200], [200, 201], [300, 301]] as const) {
    const l = layout(n("/", "folder", files(count)), new Set());
    assert.deepEqual([l.nodes.length, l.total], [drawn, drawn], `${count} files`);
    assert.equal(l.nodes.some((p) => p.heldBack), false, `${count} files: nothing held back`);
  }
  assert.equal(MAX_CARDS, 200);
  const root = layout(n("/", "folder", files(300)), new Set()).nodes[0];
  assert.deepEqual([root.collapsed, root.heldBack], [false, false], "the root is open");
});

test("the root's family counts against the budget: everything below it stays closed, and an open child is held back", () => {
  const project = n("/", "folder", [folder("a", 3), ...files(249)]);
  const l = layout(project, new Set(["a"]));
  const a = l.nodes.find((p) => p.id === "a")!;
  assert.equal(l.nodes.length, 251, "the root and its 250 children");
  assert.deepEqual([a.collapsed, a.plus, a.heldBack], [true, 3, true], "a is open in the set, drawn closed by the budget, with its +3");
  assert.equal(l.total, 254);
  assert.equal(l.nodes.filter((p) => p.heldBack).length, 1);
  const closed = layout(project, new Set()).nodes.find((p) => p.id === "a")!;
  assert.deepEqual([closed.collapsed, closed.heldBack], [true, false], "a card the user closed is never held back");
});

test("a focused file of 250 functions draws all 251 cards", () => {
  const file = n("big.py", "file", Array.from({ length: 250 }, (_, i) => n(`big.py::f${i}`, "function")));
  const l = layout(file, new Set());
  assert.deepEqual([l.nodes.length, l.total], [251, 251]);
});

test("heldBack: a call card in the set that the budget closed; a leaf, or a card with nothing to open, never", () => {
  const callee = (i: number): CallTreeNode => ({ key: null, name: `c${i}`, qualname: `c${i}`, path: null, kind: "function", call_count: 1, call_sites: [], marks: [], children: [] });
  const f = n("f", "function");
  const project = n("/", "folder", [n("a.py", "file", [f, n("leaf", "function")])]);
  const callsOf = (id: string): Calls => (id === "f" ? { state: "known", children: Array.from({ length: 199 }, (_, i) => callee(i)) } : { state: "unknown" });
  const l = layout(project, new Set(["a.py", "f", "leaf"]), new Set(), callsOf);
  const by = (id: string) => l.nodes.find((p) => p.id === id)!;
  assert.deepEqual([by("f").heldBack, by("f").collapsed], [true, true], "4 + 199 > 200");
  assert.deepEqual([by("leaf").heldBack, by("leaf").collapsed], [false, false], "it has nothing to open: the budget has nothing to hold back");
  assert.equal(by("a.py").heldBack, false);
});

test("an open card whose children do not all fit is drawn closed with its +N, and so is every open card after it; what fits before it stays whole", () => {
  const project = n("/", "folder", [folder("a", 90), folder("b", 90), folder("c", 5)]);
  const l = layout(project, new Set(["a", "b", "c"]));
  const by = (id: string) => l.nodes.find((p) => p.id === id)!;
  // /, a, b, c = 4; a's 90 = 94; b's 90 = 184; c's 5 would be 189: fits. A fourth family is what does not
  assert.equal(l.nodes.length, 189);
  const tight = layout(n("/", "folder", [folder("a", 90), folder("b", 90), folder("c", 30), folder("d", 2)]), new Set(["a", "b", "c", "d"]));
  const t = (id: string) => tight.nodes.find((p) => p.id === id)!;
  assert.deepEqual([t("a").collapsed, t("b").collapsed], [false, false], "the first two fit (5 + 90 + 90 = 185 promised)");
  assert.deepEqual([t("c").collapsed, t("c").plus], [true, 30], "c's 30 would be 215: drawn closed, with its +30");
  assert.deepEqual([t("d").collapsed, t("d").plus], [true, 2], "d would fit (187), but it is after the first refusal: closed too");
  assert.equal(tight.nodes.filter((p) => p.id.startsWith("c/") || p.id.startsWith("d/")).length, 0, "none of their children is drawn: never half a family");
  assert.equal(tight.total, 5 + 90 + 90 + 30 + 2);
  assert.equal(by("a").collapsed, false);
});

test("the cards a family has not drawn yet count: a second open card cannot spend what its siblings were promised", () => {
  // the root's 150 children are accepted (151); the first of them, open with 100 more, would make 251: refused, though only 2 cards are placed when it is asked
  const project = n("/", "folder", [folder("first", 100), ...files(149)]);
  const l = layout(project, new Set(["first"]));
  assert.equal(l.nodes.length, 150 + 1);
  assert.deepEqual([l.nodes.find((p) => p.id === "first")!.collapsed, l.nodes.find((p) => p.id === "first")!.plus], [true, 100]);
  assert.equal(l.total, 251);
});

test("total is the card count without the budget; with room it equals what is drawn; the root's own family counts", () => {
  const small = layout(n("/", "folder", [folder("a", 3)]), new Set(["a"]));
  assert.deepEqual([small.nodes.length, small.total], [5, 5]);
});

test("the call cards of an open function are part of its family: they count against the budget, in the same card", () => {
  const callee = (i: number): CallTreeNode => ({ key: null, name: `c${i}`, qualname: `c${i}`, path: null, kind: "function", call_count: 1, call_sites: [], marks: [], children: [] });
  const f = n("f", "function");
  const project = n("/", "folder", [n("a.py", "file", [f])]);
  const callsOf = (n: number) => (id: string): Calls => (id === "f" ? { state: "known", children: Array.from({ length: n }, (_, i) => callee(i)) } : { state: "unknown" });
  const fits = layout(project, new Set(["a.py", "f"]), new Set(), callsOf(197)); // 3 + 197 = 200
  assert.deepEqual([fits.nodes.length, fits.total], [200, 200]);
  const over = layout(project, new Set(["a.py", "f"]), new Set(), callsOf(198));
  assert.deepEqual([over.nodes.length, over.total], [3, 201], "f is drawn closed, with its +198");
  assert.deepEqual([over.nodes.at(-1)!.collapsed, over.nodes.at(-1)!.plus], [true, 198]);
});
