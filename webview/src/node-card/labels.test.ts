import assert from "node:assert/strict";
import { test } from "node:test";

import { type CardFacts, cardLabel, linesPhrase, timesPhrase } from "./labels.ts";

const node = (over: Partial<CardFacts> = {}): CardFacts => ({ role: "node", kind: "function", name: "checkout", docs: 0, lines: [], unknown: false, canOpen: false, collapsed: true, syntaxError: false, ...over });
const call = (over: Partial<CardFacts> = {}): CardFacts => node({ role: "call", name: "reserve_stock", lines: [16, 31], canOpen: true, collapsed: true, ...over });
const none = { selected: false, cursor: false };

// the table of 09 §6: what a screen reader says of a card
const table: { name: string; card: CardFacts; state?: { selected: boolean; cursor: boolean }; expect: string }[] = [
  { name: "a function", card: node(), expect: "function checkout" },
  { name: "selected, with a doc and the cursor in it (the row of 09 §6, without its level)", card: node({ docs: 1 }), state: { selected: true, cursor: true }, expect: "function checkout, selected, 1 doc, cursor here" },
  { name: "two docs", card: node({ docs: 2 }), expect: "function checkout, 2 docs" },
  { name: "a class that is open", card: node({ kind: "class", name: "Order", canOpen: true, collapsed: false }), expect: "class Order, expanded" },
  { name: "a file that did not parse", card: node({ kind: "file", name: "api.py", syntaxError: true }), expect: "file api.py, syntax error" },
  { name: "a call card (09 §6): called twice, where, collapsed", card: call(), expect: "call to function reserve_stock, called twice, lines 16 and 31, collapsed" },
  { name: "a call made once", card: call({ lines: [16] }), expect: "call to function reserve_stock, called once, line 16, collapsed" },
  { name: "a call made in three places", card: call({ lines: [16, 31, 40] }), expect: "call to function reserve_stock, called 3 times, lines 16, 31 and 40, collapsed" },
  { name: "a call that is open", card: call({ collapsed: false }), expect: "call to function reserve_stock, called twice, lines 16 and 31, expanded" },
  { name: "a call with nothing under it says neither", card: call({ canOpen: false }), expect: "call to function reserve_stock, called twice, lines 16 and 31" },
  { name: "a selected call with docs and the cursor in its callee", card: call({ docs: 1 }), state: { selected: true, cursor: true }, expect: "call to function reserve_stock, selected, called twice, lines 16 and 31, 1 doc, cursor here, collapsed" },
  { name: "a callee the project does not know", card: call({ unknown: true, canOpen: false, lines: [4] }), expect: "call to function reserve_stock, not in the index, called once, line 4" },
];
for (const t of table) test(`card label: ${t.name}`, () => assert.equal(cardLabel(t.card, t.state ?? none), t.expect));

test("how often and where", () => {
  assert.deepEqual([1, 2, 3, 10].map(timesPhrase), ["once", "twice", "3 times", "10 times"]);
  assert.equal(linesPhrase([16]), "line 16");
  assert.equal(linesPhrase([16, 16]), "line 16", "a line is said once");
  assert.equal(linesPhrase([16, 31]), "lines 16 and 31");
  assert.equal(linesPhrase([16, 31, 40, 52]), "lines 16, 31, 40 and 52");
});
