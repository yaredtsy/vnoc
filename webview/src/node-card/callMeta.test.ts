import assert from "node:assert/strict";
import { test } from "node:test";

import type { CallSite } from "@vnoc/shared/messages.ts";
import { callMeta } from "./callMeta.ts";
import { metaChars } from "./rows.ts";

const at = (line: number, path: string | null): CallSite => ({ start: { line: line - 1, character: 4 }, end: { line: line - 1, character: 9 }, path });

test("the line holds as many characters as the card leaves it: (card − where the name starts − the right padding) / one monospace width", () => {
  assert.equal(metaChars(), 36);
});

const sitesTable: { name: string; sites: CallSite[]; parent: string | null; texts: string[]; more: number; links: boolean[] }[] = [
  { name: "one site in the caller's file: `line 16`", sites: [at(16, "app/order.py")], parent: "app/order.py", texts: ["line 16"], more: 0, links: [true] },
  { name: "two sites in the caller's file", sites: [at(16, "app/order.py"), at(31, "app/order.py")], parent: "app/order.py", texts: ["line 16", "line 31"], more: 0, links: [true, true] },
  { name: "a site in another file (a call in an inherited constructor): the file's name and the line", sites: [at(12, "pkg/base.py")], parent: "pkg/child.py", texts: ["base.py:12"], more: 0, links: [true] },
  { name: "more than two: the first two and `N more`", sites: [at(1, "a.py"), at(2, "a.py"), at(3, "a.py"), at(4, "a.py"), at(5, "a.py")], parent: "a.py", texts: ["line 1", "line 2"], more: 3, links: [true, true] },
  { name: "a site with no file: `line 16 ?`, not a link", sites: [at(16, null)], parent: "a.py", texts: ["line 16 ?"], more: 0, links: [false] },
  { name: "no sites (the root, or a callee the driver gave none for)", sites: [], parent: "a.py", texts: [], more: 0, links: [] },
];
for (const row of sitesTable) {
  test(`callMeta sites: ${row.name}`, () => {
    const meta = callMeta(row.sites, row.parent, null, metaChars());
    assert.deepEqual(meta.sites.map((s) => s.text), row.texts);
    assert.equal(meta.more, row.more);
    assert.deepEqual(meta.sites.map((s) => s.site !== null), row.links);
  });
}

// the callee's file name is shown whole or not at all: counted in characters, with 36 to spend
const rightTable: { name: string; sites: CallSite[]; right: string; shown: boolean; left: number }[] = [
  { name: "`↳ line 16` (9) + 2 + `inventory.py` (12) = 23", sites: [at(16, "a.py")], right: "inventory.py", shown: true, left: 9 },
  { name: "`↳ line 16 · line 31` (19) + 2 + `inventory.py` (12) = 33: fits", sites: [at(16, "a.py"), at(31, "a.py")], right: "inventory.py", shown: true, left: 19 },
  { name: "19 + 2 + `payment_gateway.py` (18) = 39: does not fit, so it is hidden, not cut", sites: [at(16, "a.py"), at(31, "a.py")], right: "payment_gateway.py", shown: false, left: 19 },
  { name: "exactly 36: 19 + 2 + 15 fits (the last character counts)", sites: [at(16, "a.py"), at(31, "a.py")], right: "x".repeat(15), shown: true, left: 19 },
  { name: "37: one character more does not", sites: [at(16, "a.py"), at(31, "a.py")], right: "x".repeat(16), shown: false, left: 19 },
  { name: "`↳ line 1 · line 2 · 3 more ▾` (28) + 2 + `x.py` (4) = 34: fits", sites: [at(1, "a.py"), at(2, "a.py"), at(3, "a.py"), at(4, "a.py"), at(5, "a.py")], right: "x.py", shown: true, left: 28 },
  { name: "28 + 2 + `utils.py` (8) = 38: hidden", sites: [at(1, "a.py"), at(2, "a.py"), at(3, "a.py"), at(4, "a.py"), at(5, "a.py")], right: "utils.py", shown: false, left: 28 },
  { name: "a mark takes the same slot and the same rule: `… calls not loaded` (18) beside one site (9): 29 fits", sites: [at(16, "a.py")], right: "… calls not loaded", shown: true, left: 9 },
  { name: "`… calls not loaded` beside two sites (19): 39, hidden", sites: [at(16, "a.py"), at(31, "a.py")], right: "… calls not loaded", shown: false, left: 19 },
  { name: "no sites: the right side has the whole line", sites: [], right: "not in the index", shown: true, left: 0 },
];
for (const row of rightTable) {
  test(`callMeta right: ${row.name}`, () => {
    const meta = callMeta(row.sites, "a.py", row.right, metaChars());
    assert.equal(meta.leftLength, row.left);
    assert.equal(meta.right, row.shown ? row.right : null);
  });
}

test("nothing to put on the right: nothing", () => assert.equal(callMeta([at(1, "a.py")], "a.py", null, 36).right, null));
