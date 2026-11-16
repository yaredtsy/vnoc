import assert from "node:assert/strict";
import test from "node:test";

import { type Copy, adopt, busyText, busyWith, closeButtons, closeNotice, countingDrafts, countsOnClose, emptyCopy, failedNotice, fromCopy, isChanged, isEmptyCopy, readCopy, readDraft, readDrafts, reopenButton, savedAs, settle, sourceOf, startButtons, startNotice, startText, withPage } from "./drafts.ts";
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

const notices: { n: number; from: "canvas" | "docs" | "both" | undefined; closed: string; failed: string }[] = [
  { n: 1, from: undefined, closed: "1 unsaved draft from the canvas", failed: "1 could not be saved: Reopen Canvas to see it" },
  { n: 1, from: "canvas", closed: "1 unsaved draft from the canvas", failed: "1 could not be saved: Reopen Canvas to see it" },
  { n: 2, from: "canvas", closed: "2 unsaved drafts from the canvas", failed: "2 could not be saved: Reopen Canvas to see them" },
  { n: 10, from: "canvas", closed: "10 unsaved drafts from the canvas", failed: "10 could not be saved: Reopen Canvas to see them" },
  { n: 1, from: "docs", closed: "1 unsaved draft from the docs tab", failed: "1 could not be saved: Reopen Docs Tab to see it" },
  { n: 3, from: "docs", closed: "3 unsaved drafts from the docs tab", failed: "3 could not be saved: Reopen Docs Tab to see them" },
  { n: 2, from: "both", closed: "2 unsaved drafts from V-NOC", failed: "2 could not be saved: Reopen to see them" },
  { n: 1, from: "both", closed: "1 unsaved draft from V-NOC", failed: "1 could not be saved: Reopen to see it" },
];
for (const r of notices) {
  test(`the notifications for ${r.n} from ${r.from ?? "(the default)"}`, () => {
    assert.equal(closeNotice(r.n, r.from), r.closed);
    assert.equal(startNotice(r.n, r.from), r.closed, "at start it is the same sentence");
    assert.equal(failedNotice(r.n, r.from), r.failed);
  });
}

test("the buttons: at close Save All, the reopen button of the page, and Discard; at start only the last two", () => {
  assert.deepEqual(closeButtons("canvas"), ["Save All", "Reopen Canvas", "Discard"]);
  assert.deepEqual(startButtons("canvas"), ["Reopen Canvas", "Discard"]);
  assert.deepEqual(closeButtons("docs"), ["Save All", "Reopen Docs Tab", "Discard"]);
  assert.deepEqual(startButtons("both"), ["Reopen", "Discard"]);
  assert.deepEqual([reopenButton("canvas"), reopenButton("docs"), reopenButton("both")], ["Reopen Canvas", "Reopen Docs Tab", "Reopen"]);
});

test("which pages a notification speaks for", () => {
  assert.equal(sourceOf([]), null);
  assert.equal(sourceOf(["canvas"]), "canvas");
  assert.equal(sourceOf(["docs"]), "docs");
  assert.equal(sourceOf(["canvas", "docs"]), "both");
  assert.equal(sourceOf(["docs", "canvas"]), "both");
});

// ── the copy of both pages, and one draft per doc (§8, 9.6) ───────────────────────────────────────────────────────────

const docDraft = (path: string, over: Partial<Draft> = {}): Draft => ({ ...draft(path, { path, card: "node-1", kind: "doc", prefix: "", base: "# T\n\ntext\n", range: { start: { line: 0, character: 0 }, end: { line: 3, character: 0 } } }), text: "# T\n\ntext\n", ...over });
const copyWith = (canvas: Draft[], docs: Draft[]): Copy => ({ canvas, docs });
const everythingOpen = () => true;

const busy: { name: string; copy: Copy; path: string; asking: "canvas" | "docs"; open: (p: "canvas" | "docs") => boolean; busy: boolean }[] = [
  { name: "no drafts", copy: emptyCopy(), path: "/r/.vnoc/nodes/AA/X/a.md", asking: "docs", open: everythingOpen, busy: false },
  { name: "the canvas has a draft of this doc, the docs tab asks", copy: copyWith([docDraft("/r/.vnoc/nodes/AA/X/a.md")], []), path: "/r/.vnoc/nodes/AA/X/a.md", asking: "docs", open: everythingOpen, busy: true },
  { name: "the docs tab has it, the canvas asks", copy: copyWith([], [docDraft("/r/.vnoc/nodes/AA/X/a.md")]), path: "/r/.vnoc/nodes/AA/X/a.md", asking: "canvas", open: everythingOpen, busy: true },
  { name: "the asking page's own draft does not make it busy (the page's own store says that)", copy: copyWith([docDraft("/r/.vnoc/nodes/AA/X/a.md")], []), path: "/r/.vnoc/nodes/AA/X/a.md", asking: "canvas", open: everythingOpen, busy: false },
  { name: "another doc", copy: copyWith([docDraft("/r/.vnoc/nodes/AA/X/b.md")], []), path: "/r/.vnoc/nodes/AA/X/a.md", asking: "docs", open: everythingOpen, busy: false },
  { name: "a function draft is not a doc draft, whatever its path", copy: copyWith([draft("a", { path: "/r/.vnoc/nodes/AA/X/a.md" })], []), path: "/r/.vnoc/nodes/AA/X/a.md", asking: "docs", open: everythingOpen, busy: false },
  { name: "the canvas is closed (its draft waits for an answer): nobody is editing it", copy: copyWith([docDraft("/r/.vnoc/nodes/AA/X/a.md")], []), path: "/r/.vnoc/nodes/AA/X/a.md", asking: "docs", open: (p) => p !== "canvas", busy: false },
  { name: "an untouched draft still holds the doc", copy: copyWith([docDraft("/r/.vnoc/nodes/AA/X/a.md")], []), path: "/r/.vnoc/nodes/AA/X/a.md", asking: "docs", open: everythingOpen, busy: true },
];
for (const r of busy) test(`one draft per doc, across pages: ${r.name}`, () => assert.equal(busyWith(r.copy, r.path, r.asking, r.open), r.busy));

test("what the panel says when Edit was refused as busy", () => {
  assert.equal(busyText("docs"), "Already being edited in the canvas");
  assert.equal(busyText("canvas"), "Already being edited in the docs tab");
});

test("a doc draft counts like any other, and its kind survives being read back", () => {
  const typedDoc = docDraft("/r/.vnoc/nodes/AA/X/a.md", { text: "# T\n\nnew text\n" });
  assert.equal(countsOnClose(docDraft("/r/.vnoc/nodes/AA/X/a.md")), false, "untouched: the file's own text");
  assert.equal(countsOnClose(typedDoc), true);
  assert.deepEqual(readDraft(typedDoc), typedDoc);
  assert.equal(readDraft(typedDoc)?.kind, "doc");
  assert.equal(readDraft({ ...typedDoc, kind: "other" })?.kind, undefined, "a kind this version does not know is a function's draft");
  assert.equal("kind" in (readDraft(typed("a")) ?? {}), false, "a function's draft has none");
});

test("the copy as workspaceState held it: a list per page; 9.5c's plain list was the canvas's", () => {
  assert.deepEqual(readCopy(undefined), emptyCopy());
  assert.deepEqual(readCopy("x"), emptyCopy());
  assert.deepEqual(readCopy([typed("a")]), { canvas: [typed("a")], docs: [] });
  assert.deepEqual(readCopy({ canvas: [typed("a"), { key: "broken" }], docs: [docDraft("/d.md")] }), { canvas: [typed("a")], docs: [docDraft("/d.md")] });
  assert.deepEqual(readCopy({ docs: "nope" }), emptyCopy());
});

test("replacing one page's list keeps the other's; both empty is an empty copy", () => {
  const both = copyWith([typed("a")], [docDraft("/d.md")]);
  assert.deepEqual(withPage(both, "canvas", []), copyWith([], [docDraft("/d.md")]));
  assert.deepEqual(withPage(both, "docs", [docDraft("/e.md")]), copyWith([typed("a")], [docDraft("/e.md")]));
  assert.equal(isEmptyCopy(withPage(withPage(both, "canvas", []), "docs", [])), true);
  assert.equal(isEmptyCopy(both), false);
  assert.equal(isEmptyCopy(copyWith([], [docDraft("/d.md")])), false);
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
