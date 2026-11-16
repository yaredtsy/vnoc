import assert from "node:assert/strict";
import { test } from "node:test";

import { SAVE_ANYWAY, afterLocate, answerForAlready, docLocate, endOf, firstStep, idQuestion, nodeName, outcomeAfterApply, wholeRange, withDraft } from "./saveDraft.ts";

// docs/17-edit-in-canvas.md §6, the table of the three cases, and the "already saved" rule of 9.4.

const BASE = "def f():\n    return 1";
const WRITTEN = "def f():\n    return 2";
const OTHER = "def f():\n    return 3";

const first: { name: string; facts: Parameters<typeof firstStep>[0]; expect: ReturnType<typeof firstStep> }[] = [
  { name: "the version is the draft's and the range holds base: write", facts: { draftVersion: 4, version: 4, current: BASE, base: BASE, written: WRITTEN }, expect: "write" },
  { name: "the version is the draft's and the range holds what Save would write: already saved", facts: { draftVersion: 4, version: 4, current: WRITTEN, base: BASE, written: WRITTEN }, expect: "already" },
  { name: "the version is the draft's and the range holds something else: a conflict, nothing is written over it", facts: { draftVersion: 4, version: 4, current: OTHER, base: BASE, written: WRITTEN }, expect: "conflict" },
  { name: "the version moved on: ask the core where the function is", facts: { draftVersion: 4, version: 5, current: BASE, base: BASE, written: WRITTEN }, expect: "locate" },
  { name: "the version moved on and the old range holds other text: still ask, the function may be elsewhere", facts: { draftVersion: 4, version: 5, current: "x = 1", base: BASE, written: WRITTEN }, expect: "locate" },
  { name: "an older version than the draft's is a version that moved", facts: { draftVersion: 4, version: 3, current: BASE, base: BASE, written: WRITTEN }, expect: "locate" },
  { name: "line endings count: the same text with other endings is other text", facts: { draftVersion: 4, version: 4, current: BASE.replace("\n", "\r\n"), base: BASE, written: WRITTEN }, expect: "conflict" },
];
for (const r of first) test(`firstStep: ${r.name}`, () => assert.equal(firstStep(r.facts), r.expect));

const located: { name: string; facts: Parameters<typeof afterLocate>[0]; expect: ReturnType<typeof afterLocate> }[] = [
  { name: "moved, and as it was: write at the new range (an edit elsewhere in the file)", facts: { found: true, sentVersion: 5, version: 5, current: BASE, base: BASE, written: WRITTEN }, expect: "write" },
  { name: "the text has no such function: gone", facts: { found: false, sentVersion: 5, version: 5, current: "", base: BASE, written: WRITTEN }, expect: "gone" },
  { name: "gone is gone even if the document moved on while the core looked", facts: { found: false, sentVersion: 5, version: 6, current: "", base: BASE, written: WRITTEN }, expect: "gone" },
  { name: "found, but the function changed: a conflict", facts: { found: true, sentVersion: 5, version: 5, current: OTHER, base: BASE, written: WRITTEN }, expect: "conflict" },
  { name: "the document moved on while the core looked: a conflict, whatever is at the range now (§6's re-check)", facts: { found: true, sentVersion: 5, version: 6, current: BASE, base: BASE, written: WRITTEN }, expect: "conflict" },
  { name: "found, and it holds what Save would write: already saved", facts: { found: true, sentVersion: 5, version: 5, current: WRITTEN, base: BASE, written: WRITTEN }, expect: "already" },
  { name: "already saved does not beat the re-check: the text it was read from is not there any more", facts: { found: true, sentVersion: 5, version: 6, current: WRITTEN, base: BASE, written: WRITTEN }, expect: "conflict" },
];
for (const r of located) test(`afterLocate: ${r.name}`, () => assert.equal(afterLocate(r.facts), r.expect));

test("already saved answers saved, or written when the document has unsaved changes; it writes nothing either way", () => {
  assert.equal(answerForAlready(false), "saved");
  assert.equal(answerForAlready(true), "written");
});

test("outcomeAfterApply: refused is a conflict; applied and saved is saved; applied and not saved is written (E4)", () => {
  assert.equal(outcomeAfterApply(false, false), "conflict");
  assert.equal(outcomeAfterApply(false, true), "conflict", "a save that says yes after a refusal cannot be taken for a write");
  assert.equal(outcomeAfterApply(true, true), "saved");
  assert.equal(outcomeAfterApply(true, false), "written");
});

test("withDraft puts the draft in place of the range, and nothing else moves", () => {
  const file = "a = 1\n\ndef f():\n    return 1\n\nb = 2\n";
  const from = file.indexOf("def f");
  const to = file.indexOf("return 1") + "return 1".length;
  assert.equal(withDraft(file, from, to, "def f():\n    return 2"), "a = 1\n\ndef f():\n    return 2\n\nb = 2\n");
  assert.equal(withDraft(file, from, from, "# new\n"), "a = 1\n\n# new\ndef f():\n    return 1\n\nb = 2\n", "an empty range inserts");
});

// §7: the question about an ID the draft takes away names the node by its own name.
const names: { key: string; name: string }[] = [
  { key: "app/orders.py::parse_order", name: "parse_order" },
  { key: "app/orders.py::Order.parse", name: "parse" },
  { key: "app/orders.py::Outer.Inner.method", name: "method" },
  { key: "app/orders.py::parse_order#2", name: "parse_order" },
  { key: "app/orders.py::Order.parse#3", name: "parse" },
  { key: "app/orders.py::Order", name: "Order" },
];
for (const r of names) test(`the question names ${r.key} as ${r.name}`, () => assert.equal(nodeName(r.key), r.name));

test("the question's text, and the button", () => {
  assert.equal(idQuestion("app/orders.py::Order.parse"), "This removes parse's ID: its docs will be Missing.");
  assert.equal(SAVE_ANYWAY, "Save Anyway");
});

// §8: a doc's draft is of the whole file; `locate` for a doc answers the whole document as it is now.
const ends: { name: string; text: string; end: { line: number; character: number } }[] = [
  { name: "empty", text: "", end: { line: 0, character: 0 } },
  { name: "one line, no newline", text: "# Title", end: { line: 0, character: 7 } },
  { name: "a trailing newline: the last line is empty", text: "# Title\n", end: { line: 1, character: 0 } },
  { name: "two lines", text: "a\nbc", end: { line: 1, character: 2 } },
  { name: "CRLF counts once", text: "a\r\nbc\r\n", end: { line: 2, character: 0 } },
  { name: "a lone CR ends a line, as VS Code counts", text: "a\rb", end: { line: 1, character: 1 } },
  { name: "blank lines", text: "a\n\n\nb", end: { line: 3, character: 1 } },
  { name: "frontmatter and a body", text: "---\nnode: X\n---\n# T\n\ntext\n", end: { line: 6, character: 0 } },
];
for (const r of ends) test(`the end of a doc: ${r.name}`, () => assert.deepEqual(endOf(r.text), r.end));

test("a doc's whole range starts at 0:0 and ends at the end of the text", () => {
  assert.deepEqual(wholeRange("a\nbc"), { start: { line: 0, character: 0 }, end: { line: 1, character: 2 } });
});

test("locate for a doc finds the whole document it is given, whatever the key", async () => {
  assert.deepEqual(await docLocate("/abs/overview.md", { text: "# A\n\nmore\n" }), wholeRange("# A\n\nmore\n"));
});

test("the rows of §6 with a whole-file locate: moved and unchanged writes, changed conflicts, already saved is saved", () => {
  const base = "# A\n\ntext\n";
  const written = "# A\n\nnew text\n";
  assert.equal(afterLocate({ found: true, sentVersion: 5, version: 5, current: base, base, written }), "write");
  assert.equal(afterLocate({ found: true, sentVersion: 5, version: 5, current: "# A\n\nother\n", base, written }), "conflict");
  assert.equal(afterLocate({ found: true, sentVersion: 5, version: 5, current: written, base, written }), "already");
  assert.equal(afterLocate({ found: true, sentVersion: 5, version: 6, current: base, base, written }), "conflict", "the document moved while the core looked: still the re-check");
});
