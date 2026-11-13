import assert from "node:assert/strict";
import { test } from "node:test";

import { docsCount, footerChips } from "./footerChips.ts";

const base = { collapsed: false, childCount: 0, docs: 0, link: null } as const;
const none = { selected: false, cursor: false };

test("closed with children: +N and what it says", () => {
  assert.deepEqual(footerChips({ ...base, collapsed: true, childCount: 3 }, none), [{ kind: "hidden", text: "+3", tip: "3 hidden: open to see them", faded: false }]);
});

test("open: no +N, even with children", () => assert.deepEqual(footerChips({ ...base, childCount: 3 }, none), []));
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
  const all = footerChips({ collapsed: true, childCount: 2, docs: 1, link: "marker" }, { selected: true, cursor: true });
  assert.deepEqual(all.map((c) => c.kind), ["hidden", "docs", "link", "selected", "cursor"]);
});
