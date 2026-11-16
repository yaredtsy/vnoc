import assert from "node:assert/strict";
import { test } from "node:test";

import { type Draft, type Drafts, draftFor, indexMovedOn, isChanged, newDraft, restoreDrafts, showRefusal, startText, typed, without, withDraft } from "./draftRules.ts";

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
