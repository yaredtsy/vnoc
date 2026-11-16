import assert from "node:assert/strict";
import { test } from "node:test";

import { type Draft, type Drafts, ELSEWHERE_TEXT, feedFor, markKey, newDocDraft, canSave, draftFor, editBlock, isUnsaved, showElsewhere, indexMovedOn, isChanged, newDraft, noticeAfterFileState, noticeFor, restoreDrafts, savedAs, showRefusal, stateLine, ACTION_TEXT, startText, typed, withDraft, withState, without } from "./draftRules.ts";

// The rules of a draft (docs/17-edit-in-canvas.md §4, §5, E6).

const RANGE = { start: { line: 3, character: 4 }, end: { line: 5, character: 14 } };
const CARD = { id: "a.py::C::m", key: "a.py::C::m", path: "a.py", range: RANGE };
const started = (over: Partial<Draft> = {}): Draft => ({ ...newDraft(CARD, { text: "def m(self):\n        return 1", version: 7, prefix: "    ", eol: "\n" }), ...over });

test("a new draft: editing, the text dedented, the host's version and prefix kept, `base` as it was read", () => {
  const d = started();
  assert.deepEqual([d.state, d.text, d.base, d.version, d.prefix, d.card, d.key], ["editing", "def m(self):\n    return 1", "def m(self):\n        return 1", 7, "    ", "a.py::C::m", "a.py::C::m"]);
  assert.equal(startText(d), d.text);
});

test("a new draft keeps the document's line ending", () => {
  assert.equal(started().eol, "\n");
  assert.equal(newDraft(CARD, { text: "def m(self):\r\n        return 1", version: 7, prefix: "    ", eol: "\r\n" }).eol, "\r\n");
});

test("changed: only when the text differs from where it started", () => {
  const d = started();
  assert.equal(isChanged(d), false);
  assert.equal(isChanged({ ...d, text: d.text + "\n" }), true);
  assert.equal(isChanged({ ...d, text: d.text.replace("1", "2") }), true);
  assert.equal(isChanged({ ...d, text: d.text.replace("1", "2").replace("2", "1") }), false, "typed and typed back");
});

test("changed ignores the line ending: Monaco gives back its own, the host's text may have mixed ones", () => {
  const crlf = newDraft(CARD, { text: "def m(self):\r\n        a = 1\r\n\r\n        return a", version: 1, prefix: "    ", eol: "\r\n" });
  assert.equal(isChanged(crlf), false);
  assert.equal(isChanged({ ...crlf, text: "def m(self):\r\n    a = 1\r\n\r\n    return a" }), false, "Monaco's text, every line ending CRLF");
  assert.equal(isChanged({ ...crlf, text: "def m(self):\r\n    a = 2\r\n\r\n    return a" }), true);
});

test("one draft per node: a second one for the same key is not started", () => {
  const a = withDraft({}, started());
  const b = withDraft(a, started({ card: "call:[...]", text: "other" }));
  assert.equal(b, a);
  assert.equal(Object.keys(withDraft(a, started({ key: "b.py::f", card: "b.py::f" }))).length, 2);
});

test("a draft is shown by the card that started it, not by another card on the same function", () => {
  const drafts = withDraft({}, started());
  assert.equal(draftFor(drafts, { id: "a.py::C::m", key: "a.py::C::m" })?.key, "a.py::C::m");
  assert.equal(draftFor(drafts, { id: 'call:["x","a.py::C::m"]', key: "a.py::C::m" }), undefined, "a call card of the same function");
  assert.equal(draftFor(drafts, { id: "call:[...]", key: null }), undefined, "an unknown callee");
});

test("typing changes the text while editing, and nothing else; the same text is the same object", () => {
  const drafts = withDraft({}, started());
  const next = typed(drafts, "a.py::C::m", "def m(self):\n    return 2");
  assert.equal(next["a.py::C::m"].text, "def m(self):\n    return 2");
  assert.equal(next["a.py::C::m"].base, drafts["a.py::C::m"].base);
  assert.equal(typed(next, "a.py::C::m", "def m(self):\n    return 2"), next);
  assert.equal(typed(drafts, "nope", "x"), drafts);
  for (const state of ["saving", "conflict", "gone"] as const) {
    const held: Drafts = { k: started({ key: "k", state }) };
    assert.equal(typed(held, "k", "typed anyway"), held, `${state}: the text is not changed under a save or a conflict`);
  }
});

test("discard drops the draft, and a key that has none changes nothing", () => {
  const drafts = withDraft({}, started());
  assert.deepEqual(without(drafts, "a.py::C::m"), {});
  assert.equal(without(drafts, "other"), drafts);
});

// E6: what comes back from `getState()`
test("restore: a draft comes back as it was; one that was saving comes back editing; conflict and gone stay", () => {
  const saved = { a: started({ key: "a" }), b: started({ key: "b", state: "saving" }), c: started({ key: "c", state: "conflict" }), d: started({ key: "d", state: "gone" }) };
  const back = restoreDrafts(JSON.parse(JSON.stringify(saved)));
  assert.deepEqual(Object.fromEntries(Object.entries(back).map(([k, d]) => [k, d.state])), { a: "editing", b: "editing", c: "conflict", d: "gone" });
  assert.deepEqual(back.a, saved.a, "everything else as saved: text, base, version, prefix, range, card");
});

test("restore: a draft saved before the line ending was kept has `\\n`, and one with `\\r\\n` keeps it", () => {
  const { eol: _eol, ...old } = started({ key: "a" });
  const back = restoreDrafts(JSON.parse(JSON.stringify({ a: old, b: started({ key: "b", eol: "\r\n" }) })));
  assert.equal(back.a.eol, "\n");
  assert.equal(back.b.eol, "\r\n");
});

test("restore: nothing saved, something that is not an object, an entry that is not a whole draft or is under the wrong key", () => {
  assert.deepEqual(restoreDrafts(undefined), {});
  assert.deepEqual(restoreDrafts(null), {});
  assert.deepEqual(restoreDrafts("x"), {});
  const good = started({ key: "a" });
  const { text: _text, ...noText } = good;
  const back = restoreDrafts({ a: good, noText, wrongKey: { ...good, key: "other" }, nope: 3, noRange: { ...good, key: "noRange", range: undefined }, badState: { ...good, key: "badState", state: "weird" } });
  assert.deepEqual(Object.keys(back).sort(), ["a", "badState"]);
  assert.equal(back.badState.state, "editing", "an unknown state is `editing`: the draft's text is kept");
});

test("the index moved on: a newer revision of the same run, or another run; not the same revision, not an older one", () => {
  assert.equal(indexMovedOn({ revision: 4, epoch: 1 }, { revision: 5, epoch: 1 }), true);
  assert.equal(indexMovedOn({ revision: 4, epoch: 1 }, { revision: 4, epoch: 1 }), false);
  assert.equal(indexMovedOn({ revision: 4, epoch: 1 }, { revision: 3, epoch: 1 }), false);
  assert.equal(indexMovedOn({ revision: 4, epoch: 1 }, { revision: 1, epoch: 2 }), true, "a restarted core counts from the start again");
  assert.equal(indexMovedOn(null, { revision: 1, epoch: 1 }), true);
  assert.equal(indexMovedOn({ revision: 4, epoch: 1 }, null), false);
});

test("the refusal shows while the file is dirty, and goes when it is not", () => {
  assert.equal(showRefusal(true, true), true);
  assert.equal(showRefusal(true, false), false);
  assert.equal(showRefusal(false, true), false);
});

// 9.3b: Save
test("a draft changes state, and the same state is the same object", () => {
  const drafts = withDraft({}, started());
  const saving = withState(drafts, "a.py::C::m", "saving");
  assert.equal(saving["a.py::C::m"].state, "saving");
  assert.equal(saving["a.py::C::m"].text, drafts["a.py::C::m"].text, "only the state");
  assert.equal(withState(saving, "a.py::C::m", "saving"), saving);
  assert.equal(withState(drafts, "nope", "saving"), drafts);
});

test("Save does something only while editing a draft that differs from its start", () => {
  const same = started();
  const changed = started({ text: same.text + "\n# x" });
  assert.equal(canSave(undefined), false);
  assert.equal(canSave(same), false, "nothing was typed: nothing is sent");
  assert.equal(canSave(changed), true);
  for (const state of ["saving", "conflict", "gone"] as const) assert.equal(canSave({ ...changed, state }), false, state);
});

test("the host's answer: saved, written, conflict, gone, cancelled", () => {
  assert.equal(savedAs({ cancelled: true }), "cancelled");
  assert.equal(savedAs({ saved: true }), "saved");
  assert.equal(savedAs({ written: true }), "written");
  assert.equal(savedAs({ conflict: true }), "conflict");
  assert.equal(savedAs({ gone: true }), "gone");
});

test("the line above the code says what happened, with the file's name", () => {
  assert.deepEqual(noticeFor("written", "a.py"), { kind: "written", text: "Written into a.py, not saved." });
  assert.deepEqual(noticeFor("error", "a.py", "the core is gone"), { kind: "error", text: "Could not save: the core is gone" });
  assert.equal(noticeFor("error", "a.py").text, "Could not save");
});

test("`written, not saved` goes when the file is saved, and the error does not go with the file", () => {
  const written = noticeFor("written", "a.py");
  assert.equal(noticeAfterFileState(written, true), written, "still dirty: still not saved");
  assert.equal(noticeAfterFileState(written, false), undefined);
  const error = noticeFor("error", "a.py", "the core is gone");
  assert.equal(noticeAfterFileState(error, false), error);
  assert.equal(noticeAfterFileState(undefined, false), undefined);
});

// 9.4b: conflict and gone
test("a conflict says so and offers Reload and Show Diff; a gone function says so and offers Open in Editor and Copy Draft; editing and saving say nothing", () => {
  assert.deepEqual(stateLine("conflict", "a.py"), { text: "Changed in the editor since you started", actions: ["reload", "showDiff"] });
  assert.deepEqual(stateLine("gone", "a.py"), { text: "Not found in a.py any more", actions: ["openInEditor", "copyDraft"] });
  assert.equal(stateLine("editing", "a.py"), null);
  assert.equal(stateLine("saving", "a.py"), null);
});

test("every action has a label and a tooltip; Reload's says what is lost", () => {
  for (const a of ["reload", "showDiff", "openInEditor", "copyDraft"] as const) {
    assert.ok(ACTION_TEXT[a].label.length > 0 && ACTION_TEXT[a].tip.length > 0, a);
  }
  assert.match(ACTION_TEXT.reload.tip, /lost/);
  assert.deepEqual(["reload", "showDiff", "openInEditor", "copyDraft"].map((a) => ACTION_TEXT[a as keyof typeof ACTION_TEXT].label), ["Reload", "Show Diff", "Open in Editor", "Copy Draft"]);
});

test("a draft in conflict or gone keeps its text, and cannot be saved", () => {
  const d = started({ text: "changed" });
  for (const state of ["conflict", "gone"] as const) {
    const held = withState({ k: { ...d, key: "k" } }, "k", state);
    assert.equal(held.k.text, "changed");
    assert.equal(canSave(held.k), false);
  }
});

// ── one draft per node (§7, 9.5a) ────────────────────────────────────────────────────────────────────────────────────

const CALL_CARD = { id: 'call:["x.py::main","a.py::C::m"]', key: "a.py::C::m" };

test("an Edit on a node that has a draft: the holder's own card does nothing, any other card on the node is refused and names the holder", () => {
  const drafts = withDraft({}, started());
  assert.equal(editBlock({}, { id: "a.py::C::m", key: "a.py::C::m" }), null, "no draft: Edit starts one");
  assert.deepEqual(editBlock(drafts, { id: "a.py::C::m", key: "a.py::C::m" }), { kind: "same" });
  assert.deepEqual(editBlock(drafts, CALL_CARD), { kind: "other", holder: "a.py::C::m" }, "the call card of the function the structure card is editing");
  assert.equal(editBlock(drafts, { id: "b.py::f", key: "b.py::f" }), null, "another function");
  assert.equal(editBlock(drafts, { id: "call:[...]", key: null }), null, "a callee the project does not know has nothing to edit, and nothing to be refused for");
  const byCall = withDraft({}, started({ card: CALL_CARD.id }));
  assert.deepEqual(editBlock(byCall, { id: "a.py::C::m", key: "a.py::C::m" }), { kind: "other", holder: CALL_CARD.id }, "the other way round: the call card holds it");
});

test("the refusal's line shows while the draft that refused the card is still in another card", () => {
  const drafts = withDraft({}, started());
  assert.equal(showElsewhere(true, drafts, CALL_CARD), true);
  assert.equal(showElsewhere(false, drafts, CALL_CARD), false, "the card was never refused");
  assert.equal(showElsewhere(true, {}, CALL_CARD), false, "the draft was saved or discarded: the line goes");
  assert.equal(showElsewhere(true, drafts, { id: "a.py::C::m", key: "a.py::C::m" }), false, "the holder itself is not refused by its own draft");
  assert.equal(ELSEWHERE_TEXT, "Already being edited in another card");
});

test("unsaved: changed, or saving, conflict or gone; an untouched editing draft is not", () => {
  const d = started();
  assert.equal(isUnsaved(d), false);
  assert.equal(isUnsaved({ ...d, text: d.text + "!" }), true);
  assert.equal(isUnsaved({ ...d, state: "saving" }), true);
  assert.equal(isUnsaved({ ...d, state: "conflict" }), true);
  assert.equal(isUnsaved({ ...d, state: "gone" }), true);
});

// ── what the page tells the host (9.5c) ──────────────────────────────────────────────────────────────────────────────

const other = (over: Partial<Draft> = {}): Draft => started({ key: "b.py::f", card: "b.py::f", ...over });
const feeds: { name: string; before: Drafts; after: Drafts; feed: "none" | "now" | "later" }[] = [
  { name: "the same drafts", before: withDraft({}, started()), after: withDraft({}, started()), feed: "none" },
  { name: "a draft starts", before: {}, after: withDraft({}, started()), feed: "now" },
  { name: "the last draft goes (a save, a Discard): the host must hear it, so its copy clears", before: withDraft({}, started()), after: {}, feed: "now" },
  { name: "typing: later", before: withDraft({}, started()), after: typed(withDraft({}, started()), "a.py::C::m", "def m(self):\n    return 2"), feed: "later" },
  { name: "saving", before: withDraft({}, started()), after: withState(withDraft({}, started()), "a.py::C::m", "saving"), feed: "now" },
  { name: "conflict", before: withState(withDraft({}, started()), "a.py::C::m", "saving"), after: withState(withDraft({}, started()), "a.py::C::m", "conflict"), feed: "now" },
  { name: "a state and text together: now", before: withDraft({}, started()), after: { "a.py::C::m": { ...started(), state: "gone", text: "other" } }, feed: "now" },
  { name: "one draft goes while another is typed into", before: { ...withDraft({}, started()), "b.py::f": other() }, after: { "b.py::f": other({ text: "x" }) }, feed: "now" },
  { name: "two drafts, one typed into", before: { ...withDraft({}, started()), "b.py::f": other() }, after: { ...withDraft({}, started()), "b.py::f": other({ text: "x" }) }, feed: "later" },
  { name: "a draft replaced by an equal one (a new object, the same content)", before: withDraft({}, started()), after: { "a.py::C::m": started() }, feed: "none" },
];
for (const r of feeds) test(`the page feeds the host: ${r.name}`, () => assert.equal(feedFor(r.before, r.after), r.feed));

// ── a doc's draft (§8, 9.6) ─────────────────────────────────────────────────────────────────────────────────────────

test("a new doc draft: whole file as the range, no prefix, editing, the text as the file has it, keyed by the path and held by the node", () => {
  const range = { start: { line: 0, character: 0 }, end: { line: 6, character: 0 } };
  const text = "---\nnode: X\n---\n# T\n\nbody  \n";
  const d = newDocDraft({ path: "/r/.vnoc/nodes/AA/X/a.md", node: "src/a.py::f" }, { text, version: 3, eol: "\n", range });
  assert.deepEqual([d.kind, d.key, d.path, d.card, d.prefix, d.state, d.range, d.version, d.base, d.text], ["doc", "/r/.vnoc/nodes/AA/X/a.md", "/r/.vnoc/nodes/AA/X/a.md", "src/a.py::f", "", "editing", range, 3, text, text]);
  assert.equal(isChanged(d), false);
  assert.equal(isChanged({ ...d, text: text + "more\n" }), true);
  assert.equal(newDocDraft({ path: "/d.md", node: "n" }, { text: "a\r\nb", version: 1, eol: "\r\n", range }).eol, "\r\n");
});

test("the key of a draft's marks: a card's id, or a doc's path", () => {
  assert.equal(markKey(started()), "a.py::C::m");
  assert.equal(markKey(newDocDraft({ path: "/d.md", node: "n" }, { text: "", version: 1, eol: "\n" })), "/d.md");
});

test("a doc draft is kept by the store and restored with the page like any other, kind and all", () => {
  const d = newDocDraft({ path: "/d.md", node: "n" }, { text: "# T\n", version: 1, eol: "\n", range: RANGE });
  assert.deepEqual(restoreDrafts({ "/d.md": { ...d, state: "saving" } }), { "/d.md": { ...d, state: "editing" } });
  assert.equal(draftFor({ "/d.md": d }, { id: "n", key: "/d.md" })?.kind, "doc", "the node's card id and the doc's path find it");
});
