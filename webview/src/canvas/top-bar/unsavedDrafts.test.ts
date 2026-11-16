import assert from "node:assert/strict";
import { test } from "node:test";

import type { Draft } from "../../state/draftRules.ts";
import { newDraft } from "../../state/draftRules.ts";
import { nextCard, unsavedDrafts } from "./unsavedDrafts.ts";

const at = (path: string, line: number) => ({ start: { line, character: 0 }, end: { line: line + 2, character: 10 } });
/** A draft of `key` started on `card` in `path` at `line`; `over` is what typing or a save did to it. */
function draft(key: string, path: string, line: number, over: Partial<Draft> = {}, card = key): Draft {
  return { ...newDraft({ id: card, key, path, range: at(path, line) }, { text: "def f():\n    return 1", version: 1, prefix: "", eol: "\n" }), ...over };
}
const typed = (d: Draft): Draft => ({ ...d, text: d.text + "\n# typed" });
const held = (...ds: Draft[]) => Object.fromEntries(ds.map((d) => [d.key, d]));

test("no drafts, or only untouched ones: no chip", () => {
  assert.equal(unsavedDrafts({}), null);
  assert.equal(unsavedDrafts(held(draft("a.py::f", "a.py", 0))), null, "editing, nothing typed");
});

// each row: the drafts, then the count, the label and the cards
const table: { name: string; drafts: Draft[]; count: number; label: string; cards: string[] }[] = [
  { name: "one changed draft", drafts: [typed(draft("a.py::f", "a.py", 3))], count: 1, label: "1 unsaved draft", cards: ["a.py::f"] },
  { name: "typed and typed back: not counted", drafts: [draft("a.py::f", "a.py", 3, { text: "def f():\n    return 1" })], count: 0, label: "", cards: [] },
  { name: "saving counts", drafts: [draft("a.py::f", "a.py", 3, { state: "saving" })], count: 1, label: "1 unsaved draft", cards: ["a.py::f"] },
  { name: "a conflict counts, changed or not", drafts: [draft("a.py::f", "a.py", 3, { state: "conflict" })], count: 1, label: "1 unsaved draft", cards: ["a.py::f"] },
  { name: "a function that is gone counts", drafts: [draft("a.py::f", "a.py", 3, { state: "gone" })], count: 1, label: "1 unsaved draft", cards: ["a.py::f"] },
  { name: "three: the label says drafts", drafts: [typed(draft("a.py::f", "a.py", 3)), draft("a.py::g", "a.py", 9, { state: "conflict" }), typed(draft("b.py::h", "b.py", 1))], count: 3, label: "3 unsaved drafts", cards: ["a.py::f", "a.py::g", "b.py::h"] },
  { name: "an untouched one among them is left out", drafts: [typed(draft("a.py::f", "a.py", 3)), draft("a.py::g", "a.py", 9), typed(draft("b.py::h", "b.py", 1))], count: 2, label: "2 unsaved drafts", cards: ["a.py::f", "b.py::h"] },
  { name: "the order is by file, then line, whatever the order they were started in", drafts: [typed(draft("b.py::h", "b.py", 1)), typed(draft("a.py::g", "a.py", 9)), typed(draft("a.py::f", "a.py", 3))], count: 3, label: "3 unsaved drafts", cards: ["a.py::f", "a.py::g", "b.py::h"] },
  { name: "the card is the one that holds the draft: a call card, not the structure card", drafts: [typed(draft("a.py::f", "a.py", 3, {}, 'call:["x.py::main","a.py::f"]'))], count: 1, label: "1 unsaved draft", cards: ['call:["x.py::main","a.py::f"]'] },
];
for (const row of table) {
  test(`the bar: ${row.name}`, () => {
    const got = unsavedDrafts(held(...row.drafts));
    if (row.count === 0) return assert.equal(got, null);
    assert.deepEqual([got?.count, got?.label, got?.cards], [row.count, row.label, row.cards]);
  });
}

test("a click walks the cards in order and starts again after the last", () => {
  const cards = ["a", "b", "c"];
  assert.equal(nextCard(cards, null), "a", "never clicked");
  assert.equal(nextCard(cards, "a"), "b");
  assert.equal(nextCard(cards, "b"), "c");
  assert.equal(nextCard(cards, "c"), "a", "after the last");
  assert.equal(nextCard(cards, "gone"), "a", "the last one clicked has no unsaved draft any more");
  assert.equal(nextCard(["only"], "only"), "only");
  assert.equal(nextCard([], null), null);
});
