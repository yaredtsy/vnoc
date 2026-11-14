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
