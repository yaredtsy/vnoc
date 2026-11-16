import assert from "node:assert/strict";
import test from "node:test";

import { CLOSE_BUTTONS, START_BUTTONS, adopt, closeNotice, countingDrafts, countsOnClose, failedNotice, fromCopy, isChanged, readDraft, readDrafts, savedAs, settle, startNotice, startText } from "./drafts.ts";
import type { Draft } from "./messages.ts";

// docs/17-edit-in-canvas.md §7, 9.5c: the host's copy of the drafts, and what it does when the canvas is closed or at start.

const RANGE = { start: { line: 3, character: 4 }, end: { line: 5, character: 14 } };
function draft(key: string, over: Partial<Draft> = {}): Draft {
  const d = { key, card: key, path: "a.py", range: RANGE, version: 7, base: "def m(self):\n        return 1", prefix: "    ", eol: "\n" as const, state: "editing" as const };
  return { ...d, text: startText(d), ...over };
}
const typed = (key: string, over: Partial<Draft> = {}): Draft => draft(key, { text: startText(draft(key)) + "\n# typed", ...over });

// ── which drafts count ────────────────────────────────────────────────────────────────────────────────────────────────

const counts: { name: string; draft: Draft; counts: boolean }[] = [
  { name: "editing, nothing typed", draft: draft("a"), counts: false },
  { name: "editing, typed into", draft: typed("a"), counts: true },
  { name: "typed and typed back", draft: draft("a", { text: startText(draft("a")) + "x" }), counts: true },
  { name: "editing, its text the start's with other line endings: not changed", draft: draft("a", { base: "def m(self):\r\n        return 1", text: "def m(self):\n    return 1", eol: "\r\n" }), counts: false },
  { name: "saving, typed into: the text is what a save would write, and the page may not live to see it", draft: typed("a", { state: "saving" }), counts: true },
  { name: "conflict", draft: typed("a", { state: "conflict" }), counts: true },
  { name: "conflict, text equal to the start (not possible by typing, but the state alone counts)", draft: draft("a", { state: "conflict" }), counts: true },
  { name: "gone", draft: typed("a", { state: "gone" }), counts: true },
  { name: "gone, text equal to the start", draft: draft("a", { state: "gone" }), counts: true },
];
for (const r of counts) test(`which drafts count: ${r.name}`, () => assert.equal(countsOnClose(r.draft), r.counts));

test("countingDrafts keeps the ones that count, in order", () => {
  const list = [typed("a"), draft("b"), draft("c", { state: "gone" }), draft("d")];
  assert.deepEqual(countingDrafts(list).map((d) => d.key), ["a", "c"]);
  assert.deepEqual(countingDrafts([]), []);
  assert.equal(isChanged(typed("a")), true);
});

// ── what each notification says ───────────────────────────────────────────────────────────────────────────────────────

const notices: { n: number; closed: string; failed: string }[] = [
  { n: 1, closed: "1 unsaved draft from the canvas", failed: "1 could not be saved: Reopen Canvas to see it" },
  { n: 2, closed: "2 unsaved drafts from the canvas", failed: "2 could not be saved: Reopen Canvas to see them" },
  { n: 10, closed: "10 unsaved drafts from the canvas", failed: "10 could not be saved: Reopen Canvas to see them" },
];
for (const r of notices) {
  test(`the notifications for ${r.n}`, () => {
    assert.equal(closeNotice(r.n), r.closed);
    assert.equal(startNotice(r.n), r.closed, "at start it is the same sentence");
    assert.equal(failedNotice(r.n), r.failed);
  });
}

test("the buttons: at close Save All, Reopen Canvas and Discard; at start only the last two", () => {
  assert.deepEqual([...CLOSE_BUTTONS], ["Save All", "Reopen Canvas", "Discard"]);
  assert.deepEqual([...START_BUTTONS], ["Reopen Canvas", "Discard"]);
});

// ── what Save All leaves in the copy ──────────────────────────────────────────────────────────────────────────────────

test("Save All: saved and written go; a conflict and a gone function stay in that state; the rest stays as it was", () => {
  const list = [typed("saved"), typed("written"), typed("conflict"), typed("gone"), typed("cancelled"), typed("error"), draft("untouched")];
  const out = settle(list, ["saved", "written", "conflict", "gone", "cancelled", "error", undefined]);
  assert.deepEqual(out.remaining.map((d) => [d.key, d.state]), [["conflict", "conflict"], ["gone", "gone"], ["cancelled", "editing"], ["error", "editing"]]);
  assert.equal(out.failed, 4, "the K of the second notification: the ones that stay");
  assert.equal(out.remaining[0].text, typed("conflict").text, "a draft that stays keeps its text");
});

test("Save All: everything saved leaves an empty copy and no second notification", () => {
  assert.deepEqual(settle([typed("a"), typed("b")], ["saved", "written"]), { remaining: [], failed: 0 });
  assert.deepEqual(settle([], []), { remaining: [], failed: 0 });
});

test("Save All: a draft that was saving and got a conflict goes to conflict", () => {
  assert.deepEqual(settle([typed("a", { state: "saving" })], ["conflict"]).remaining.map((d) => d.state), ["conflict"]);
});

test("what editSave said, as an answer", () => {
  assert.deepEqual([savedAs({ saved: true }), savedAs({ written: true }), savedAs({ conflict: true }), savedAs({ gone: true }), savedAs({ cancelled: true })], ["saved", "written", "conflict", "gone", "cancelled"]);
});

// ── reading what was saved, and what a page that starts takes ─────────────────────────────────────────────────────────

test("a saved draft is read whole or not at all; an unknown state is editing, a missing eol is \\n", () => {
  assert.deepEqual(readDraft(typed("a")), typed("a"));
  assert.equal(readDraft(null), null);
  assert.equal(readDraft("draft"), null);
  assert.equal(readDraft({ ...typed("a"), range: undefined }), null);
  assert.equal(readDraft({ ...typed("a"), version: "7" }), null);
  assert.equal(readDraft({ ...typed("a"), range: { start: { line: 1 }, end: RANGE.end } }), null);
  assert.equal(readDraft({ ...typed("a"), state: "from-the-future" })?.state, "editing");
  assert.equal(readDraft({ ...typed("a"), state: "saving" })?.state, "saving", "saving is kept: what it means is a page's to say");
  const { eol: _eol, ...noEol } = typed("a");
  assert.equal(readDraft(noEol)?.eol, "\n");
  assert.equal(readDraft({ ...typed("a"), eol: "\r\n" })?.eol, "\r\n");
});

test("the copy as workspaceState held it: a list of whole drafts, one per key", () => {
  assert.deepEqual(readDrafts(undefined), []);
  assert.deepEqual(readDrafts({}), []);
  assert.deepEqual(readDrafts("x"), []);
  assert.deepEqual(readDrafts([typed("a"), { key: "broken" }, null, typed("b"), typed("a", { text: "second a" })]).map((d) => [d.key, d.text.includes("typed")]), [["a", true], ["b", true]], "the broken ones are dropped, the first of a key wins");
});

test("a page that starts takes the copy by key, a draft that was saving comes back editing, a conflict stays a conflict", () => {
  const got = fromCopy([typed("a", { state: "saving" }), typed("b", { state: "conflict" }), typed("c"), typed("d", { state: "gone" })]);
  assert.deepEqual(Object.entries(got).map(([k, d]) => [k, d.state]), [["a", "editing"], ["b", "conflict"], ["c", "editing"], ["d", "gone"]]);
  assert.equal(got.a.text, typed("a").text, "with its text");
});

test("a page's own drafts win: a draft started between asking and the answer keeps its key", () => {
  const own = { a: typed("a", { text: "own" }) };
  const next = adopt(own, [typed("a", { text: "copy" }), typed("b")]);
  assert.equal(next.a.text, "own");
  assert.deepEqual(Object.keys(next), ["a", "b"]);
  assert.deepEqual(adopt({}, []), {});
});
