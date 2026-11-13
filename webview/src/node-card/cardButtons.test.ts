import assert from "node:assert/strict";
import { test } from "node:test";

import { cardButtons } from "./cardButtons.ts";

const RANGE = { start: { line: 0, character: 0 }, end: { line: 3, character: 0 } };
const ids = (node: Parameters<typeof cardButtons>[0], focusRoot = false) => cardButtons(node, focusRoot).map((b) => b.id);

// the table of 04 §3
test("folder: Focus · Add Doc", () => assert.deepEqual(ids({ id: "pkg", kind: "folder", range: null }), ["focus", "addDoc"]));
test("file: Focus · Go to Code · Add Doc, no code in the card", () => assert.deepEqual(ids({ id: "a.py", kind: "file", range: RANGE }), ["focus", "goToCode", "addDoc"]));
test("class: Focus · Go to Code · Code in Card · Add Doc", () => assert.deepEqual(ids({ id: "C", kind: "class", range: RANGE }), ["focus", "goToCode", "code", "addDoc"]));
test("function: the same four", () => assert.deepEqual(ids({ id: "f", kind: "function", range: RANGE }), ["focus", "goToCode", "code", "addDoc"]));
test("the opened folder has no buttons", () => assert.deepEqual(ids({ id: "/", kind: "folder", range: null }), []));
test("a function without a range has no code button", () => assert.deepEqual(ids({ id: "f", kind: "function", range: null }), ["focus", "goToCode", "addDoc"]));

test("the root of a focused view: Focus is there and disabled, the others are not", () => {
  const b = cardButtons({ id: "a.py", kind: "file", range: RANGE }, true);
  assert.deepEqual(b.map((x) => [x.id, x.disabled]), [["focus", true], ["goToCode", false], ["addDoc", false]]);
});

test("the opened folder stays without buttons when it is the focus root too", () => assert.deepEqual(ids({ id: "/", kind: "folder", range: null }, true), []));
