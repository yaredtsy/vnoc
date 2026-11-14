import assert from "node:assert/strict";
import { test } from "node:test";

import { docsCount, footerChips } from "./footerChips.ts";

const base = { collapsed: false, plus: 0, docs: 0, link: null, call: null } as const;
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

const merged = (count: number, lines: number[]) => ({ ...base, call: { count, callerName: "checkout_order", lines } });
test("×N on a call node that merges calls, with where: `checkout_order calls it 2 times: lines 16, 31`", () => {
  const [chip] = footerChips(merged(2, [16, 31]), none);
  assert.deepEqual(chip, { kind: "times", text: "×2", tip: "checkout_order calls it 2 times: lines 16, 31", faded: false });
});
test("one call: no ×N; two calls on one line say `line`", () => {
  assert.deepEqual(footerChips(merged(1, [16]), none), []);
  assert.equal(footerChips(merged(2, [16, 16]), none)[0].tip, "checkout_order calls it 2 times: line 16");
});
test("×N comes first, before +N, docs and the rest", () => {
  const all = footerChips({ ...merged(3, [1, 2, 3]), collapsed: true, plus: 4, docs: 1, link: "marker" }, { selected: true, cursor: false });
  assert.deepEqual(all.map((c) => c.kind), ["times", "hidden", "docs", "link", "selected"]);
});

test("×N only where the sites back the count: calls that reach a node through a merged parent are more than the places it is written, and say nothing", () => {
  assert.deepEqual(footerChips(merged(2, [14]), none), [], "two calls reach it, one line in the code");
  assert.deepEqual(footerChips(merged(4, [19, 20, 26, 27]), none).map((c) => c.text), ["×4"], "four calls, four places");
});
