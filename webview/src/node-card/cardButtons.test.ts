import assert from "node:assert/strict";
import { test } from "node:test";

import { cardButtons } from "./cardButtons.ts";

const RANGE = { start: { line: 0, character: 0 }, end: { line: 3, character: 0 } };
const ids = (node: Partial<Parameters<typeof cardButtons>[0]> & Pick<Parameters<typeof cardButtons>[0], "id" | "kind" | "range">, focusRoot = false) => cardButtons({ role: "node", key: node.id, ...node }, focusRoot).map((b) => b.id);

// the table of 04 §3
test("folder: Focus · Add Doc", () => assert.deepEqual(ids({ id: "pkg", kind: "folder", range: null }), ["focus", "addDoc"]));
test("file: Focus · Go to Code · Add Doc, no code in the card", () => assert.deepEqual(ids({ id: "a.py", kind: "file", range: RANGE }), ["focus", "goToCode", "addDoc"]));
test("class: Focus · Go to Code · Code in Card · Add Doc", () => assert.deepEqual(ids({ id: "C", kind: "class", range: RANGE }), ["focus", "goToCode", "code", "addDoc"]));
test("function: the same four", () => assert.deepEqual(ids({ id: "f", kind: "function", range: RANGE }), ["focus", "goToCode", "code", "addDoc"]));
test("the opened folder has no buttons", () => assert.deepEqual(ids({ id: "/", kind: "folder", range: null }), []));
test("a function without a range has no code button", () => assert.deepEqual(ids({ id: "f", kind: "function", range: null }), ["focus", "goToCode", "addDoc"]));

test("the root of a focused view: Focus is there and disabled, the others are not", () => {
  const b = cardButtons({ id: "a.py", kind: "file", range: RANGE, role: "node", key: "a.py" }, true);
  assert.deepEqual(b.map((x) => [x.id, x.disabled]), [["focus", true], ["goToCode", false], ["addDoc", false]]);
});

test("the opened folder stays without buttons when it is the focus root too", () => assert.deepEqual(ids({ id: "/", kind: "folder", range: null }, true), []));

// the call cards (04 §3)
const call = (key: string | null, range: typeof RANGE | null) => ({ id: "call:[...]", kind: "function" as const, range, role: "call" as const, key });
test("a call card whose callee is known: Explore · Go to Code · Code in Card", () => assert.deepEqual(ids(call("lib.py::leaf", RANGE)), ["explore", "goToCode", "code"]));
test("a known callee whose range is not known has no code button", () => assert.deepEqual(ids(call("lib.py::leaf", null)), ["explore", "goToCode"]));
test("a call card whose callee the project does not know: no buttons (its sites are on the meta line)", () => assert.deepEqual(ids(call(null, null)), []));
test("a call card never has Focus or Add Doc, and Explore is never disabled", () => {
  const b = cardButtons({ ...call("k", RANGE) }, true);
  assert.ok(!b.some((x) => x.id === "focus" || x.id === "addDoc"));
  assert.ok(b.every((x) => !x.disabled));
});

// a sidebar row (07 §3): the same buttons without Code in Card, which changes a card the row does not have. They are what the native tree's inline buttons were
// (Focus on every node, Go to Code on a file, class or function, Add Doc on every node), so nothing the tree did is missing
test("a sidebar row has the card's buttons except Code in Card", () => {
  const row = (kind: "folder" | "file" | "class" | "function") => cardButtons({ id: kind, kind, range: kind === "class" || kind === "function" ? RANGE : null, role: "node", key: kind }, false, "sidebar").map((b) => b.id);
  assert.deepEqual(row("folder"), ["focus", "addDoc"]);
  assert.deepEqual(row("file"), ["focus", "goToCode", "addDoc"]);
  assert.deepEqual(row("class"), ["focus", "goToCode", "addDoc"]);
  assert.deepEqual(row("function"), ["focus", "goToCode", "addDoc"]);
  assert.deepEqual(ids({ id: "f", kind: "function", range: RANGE }), ["focus", "goToCode", "code", "addDoc"], "the card still has it");
});

test("on the focused row Focus is there and does nothing, as on the card", () => {
  const b = cardButtons({ id: "a.py", kind: "file", range: null, role: "node", key: "a.py" }, true, "sidebar");
  assert.deepEqual(b.map((x) => [x.id, x.disabled]), [["focus", true], ["goToCode", false], ["addDoc", false]]);
});

// the code's slot by edit state (docs/17-edit-in-canvas.md §4)
const STATES = ["closed", "reading", "editing", "saving", "conflict", "gone"] as const;
const strip = (node: Parameters<typeof cardButtons>[0], state: (typeof STATES)[number], changed = false, surface: "canvas" | "sidebar" = "canvas") => cardButtons(node, false, surface, { state, changed });
const fn = { id: "a.py::f", kind: "function" as const, range: RANGE, role: "node" as const, key: "a.py::f" };
const calleeCard = { id: "call:[...]", kind: "function" as const, range: RANGE, role: "call" as const, key: "lib.py::leaf" };

const slot: Record<(typeof STATES)[number], string[]> = { closed: ["code"], reading: ["code", "edit"], editing: ["save", "discard"], saving: ["save", "discard"], conflict: ["save", "discard"], gone: ["save", "discard"] };
for (const state of STATES) {
  test(`a function card, ${state}: ${["focus", "goToCode", ...slot[state], "addDoc"].join(" · ")}`, () => assert.deepEqual(strip(fn, state).map((b) => b.id), ["focus", "goToCode", ...slot[state], "addDoc"]));
  test(`a class card, ${state}: the same slot`, () => assert.deepEqual(strip({ ...fn, kind: "class" }, state).map((b) => b.id), ["focus", "goToCode", ...slot[state], "addDoc"]));
  test(`a call card, ${state}: ${["explore", "goToCode", ...slot[state]].join(" · ")}`, () => assert.deepEqual(strip(calleeCard, state).map((b) => b.id), ["explore", "goToCode", ...slot[state]]));
}

test("Save is enabled only while editing a draft that differs from its start", () => {
  const save = (state: (typeof STATES)[number], changed: boolean) => strip(fn, state, changed).find((b) => b.id === "save")?.disabled;
  assert.equal(save("editing", false), true, "the draft equals its start");
  assert.equal(save("editing", true), false);
  for (const state of ["saving", "conflict", "gone"] as const) assert.equal(save(state, true), true, `${state}: disabled even when it differs`);
  assert.equal(strip(calleeCard, "editing", true).find((b) => b.id === "save")?.disabled, false, "on a call card too");
});

test("Discard is disabled while it saves, and in no other state", () => {
  for (const state of ["editing", "conflict", "gone"] as const) assert.equal(strip(fn, state, true).find((b) => b.id === "discard")?.disabled, false, state);
  assert.equal(strip(fn, "saving", true).find((b) => b.id === "discard")?.disabled, true);
});

test("Edit is never disabled, and nothing else changes with the state: Focus keeps its rule, Explore is never disabled", () => {
  assert.equal(strip(fn, "reading").find((b) => b.id === "edit")?.disabled, false);
  assert.equal(cardButtons(fn, true, "canvas", { state: "editing", changed: true }).find((b) => b.id === "focus")?.disabled, true);
  assert.ok(strip(calleeCard, "reading").every((b) => !b.disabled));
});

test("a card without code has no slot in any state: a file, a folder, a function without a range, a callee without a range", () => {
  for (const state of STATES) {
    assert.deepEqual(strip({ id: "a.py", kind: "file", range: RANGE, role: "node", key: "a.py" }, state).map((b) => b.id), ["focus", "goToCode", "addDoc"], `file, ${state}`);
    assert.deepEqual(strip({ id: "pkg", kind: "folder", range: null, role: "node", key: "pkg" }, state).map((b) => b.id), ["focus", "addDoc"], `folder, ${state}`);
    assert.deepEqual(strip({ ...fn, range: null }, state).map((b) => b.id), ["focus", "goToCode", "addDoc"], `no range, ${state}`);
    assert.deepEqual(strip({ ...calleeCard, range: null }, state).map((b) => b.id), ["explore", "goToCode"], `callee without range, ${state}`);
  }
  assert.deepEqual(strip({ ...calleeCard, key: null, range: null }, "editing"), [], "an unknown callee has none");
});

test("a sidebar row never shows `</>`, Edit, Save or Discard, whatever it is told", () => {
  for (const state of STATES) {
    for (const kind of ["folder", "file", "class", "function"] as const) {
      const ids = strip({ id: kind, kind, range: kind === "class" || kind === "function" ? RANGE : null, role: "node", key: kind }, state, true, "sidebar").map((b) => b.id);
      assert.ok(!ids.some((id) => ["code", "edit", "save", "discard"].includes(id)), `${kind} row, ${state}: ${ids}`);
    }
  }
});

test("the default is the card with its code closed: today's buttons", () => {
  assert.deepEqual(cardButtons(fn, false).map((b) => b.id), ["focus", "goToCode", "code", "addDoc"]);
  assert.deepEqual(cardButtons(calleeCard, false).map((b) => b.id), ["explore", "goToCode", "code"]);
});
