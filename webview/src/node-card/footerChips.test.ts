import assert from "node:assert/strict";
import { test } from "node:test";

import { docsCount, footerChips } from "./footerChips.ts";

const base = { collapsed: false, plus: 0, docs: 0, link: null, call: null, callsFooter: null } as const;
const none = { selected: false, cursor: false };

test("closed with children: +N and what it says", () => {
  assert.deepEqual(footerChips({ ...base, collapsed: true, plus: 3 }, none), [{ kind: "hidden", text: "+3", tip: "3 hidden: open to see them", faded: false }]);
});

test("open: no +N, even with children", () => assert.deepEqual(footerChips({ ...base, plus: 3 }, none), []));
test("closed with no children (a leaf): no +N", () => assert.deepEqual(footerChips({ ...base, collapsed: true }, none), []));

test("1 doc and 2 docs", () => {
  assert.equal(footerChips({ ...base, docs: 1 }, none)[0].tip, "1 doc: click to read");
  assert.equal(footerChips({ ...base, docs: 2 }, none)[0].tip, "2 docs: click to read");
  assert.equal(footerChips({ ...base, docs: 2 }, none)[0].text, "2");
  assert.equal(docsCount(1), "1 doc");
  assert.equal(docsCount(0), "0 docs");
});

test("the link shows only with docs", () => {
  assert.deepEqual(footerChips({ ...base, link: "marker" }, none), []);
  assert.deepEqual(footerChips({ ...base, docs: 1, link: "marker" }, none).map((c) => c.kind), ["docs", "link"]);
});

test("a link by ID is solid; by name it is faded and says Write ID", () => {
  const [, marker] = footerChips({ ...base, docs: 1, link: "marker" }, none);
  assert.deepEqual([marker.tip, marker.faded], ["ID in the code", false]);
  const [, name] = footerChips({ ...base, docs: 1, link: "name" }, none);
  assert.deepEqual([name.tip, name.faded], ["found by name only: Write ID to keep it on renames", true]);
});

test("● and ○ when a closed card holds the selection or the cursor", () => {
  assert.deepEqual(footerChips(base, { selected: true, cursor: false }).map((c) => [c.text, c.tip]), [["●", "the selection is inside"]]);
  assert.deepEqual(footerChips(base, { selected: false, cursor: true }).map((c) => [c.text, c.tip]), [["○", "the cursor is inside"]]);
});

test("the order: hidden, docs, link, ●, ○", () => {
  const all = footerChips({ ...base, collapsed: true, plus: 2, docs: 1, link: "marker" }, { selected: true, cursor: true });
  assert.deepEqual(all.map((c) => c.kind), ["hidden", "docs", "link", "selected", "cursor"]);
});

const merged = (lines: number[]) => ({ ...base, call: { callerName: "checkout_order", lines } });
test("×N on a call node with several sites, with where: `checkout_order calls it 2 times: lines 16, 31`", () => {
  const [chip] = footerChips(merged([16, 31]), none);
  assert.deepEqual(chip, { kind: "times", text: "×2", tip: "checkout_order calls it 2 times: lines 16, 31", faded: false });
});
test("one site: no ×N; two calls on one line say `line`", () => {
  assert.deepEqual(footerChips(merged([16]), none), []);
  assert.equal(footerChips(merged([16, 16]), none)[0].tip, "checkout_order calls it 2 times: line 16");
});
test("×N comes first, before +N, docs and the rest", () => {
  const all = footerChips({ ...merged([1, 2, 3]), collapsed: true, plus: 4, docs: 1, link: "marker" }, { selected: true, cursor: false });
  assert.deepEqual(all.map((c) => c.kind), ["times", "hidden", "docs", "link", "selected"]);
});

// The core's `call_count` is not an input: it is the calls that reach the node, once per copy of a merged parent. N is the sites.
test("×N is the number of sites, whatever the core counted: 2 sites → ×2, 1 site → none, 3 sites → ×3", () => {
  assert.deepEqual(footerChips(merged([19, 20]), none).map((c) => c.text), ["×2"], "core count 4, two sites");
  assert.deepEqual(footerChips(merged([14]), none), [], "core count 2, one site");
  assert.deepEqual(footerChips(merged([1, 2, 3]), none).map((c) => c.text), ["×3"]);
});

// the calls' state is the footer's right side, last of all
const callsRows: { name: string; footer: Parameters<typeof footerChips>[0]["callsFooter"]; expect: unknown }[] = [
  { name: "nothing to say: no chip", footer: null, expect: [] },
  { name: "calls nothing", footer: { state: "nothing" }, expect: [{ kind: "calls", calls: "nothing", text: "calls nothing", tip: "opening it shows nothing: it calls no function in this project", faded: false }] },
  { name: "fetching: a spinner, no text", footer: { state: "loading" }, expect: [{ kind: "calls", calls: "loading", text: "", tip: "reading what it calls…", faded: false }] },
  { name: "failed: couldn't read calls (Retry is drawn beside it), with the reason", footer: { state: "failed", message: "boom" }, expect: [{ kind: "calls", calls: "failed", text: "couldn't read calls", tip: "couldn't read calls: boom", faded: false }] },
  { name: "failed with no reason", footer: { state: "failed", message: "" }, expect: [{ kind: "calls", calls: "failed", text: "couldn't read calls", tip: "couldn't read calls", faded: false }] },
];
for (const row of callsRows) test(`calls state: ${row.name}`, () => assert.deepEqual(footerChips({ ...base, callsFooter: row.footer }, none), row.expect));

test("the calls' state comes after every other chip", () => {
  const all = footerChips({ ...base, collapsed: true, plus: 4, docs: 1, link: "marker", callsFooter: { state: "nothing" } }, { selected: true, cursor: true });
  assert.deepEqual(all.map((c) => c.kind), ["hidden", "docs", "link", "selected", "cursor", "calls"]);
});

test("no `calls 4`: a closed card with calls says its number once, in the +N, and has no calls chip", () => {
  const kinds = footerChips({ ...base, collapsed: true, plus: 4, callsFooter: null }, none).map((c) => [c.kind, c.text]);
  assert.deepEqual(kinds, [["hidden", "+4"]]);
});

test("the +N of a held-back card says why, and what a click does (03 §6.1)", () => {
  const plus = (node: Parameters<typeof footerChips>[0]) => footerChips(node, { selected: false, cursor: false }).find((c) => c.kind === "hidden")?.tip;
  const base = { collapsed: true, plus: 3, docs: 0, link: null, callsFooter: null, call: null } as const;
  assert.equal(plus({ ...base, heldBack: true }), "Not drawn: the canvas shows at most 200 cards. Click to focus on it.");
  assert.equal(plus({ ...base, heldBack: true, call: { callerName: "f", lines: [1] } }), "Not drawn: the canvas shows at most 200 cards. Click to explore it.");
  assert.equal(plus({ ...base, heldBack: false }), "3 hidden: open to see them");
});
