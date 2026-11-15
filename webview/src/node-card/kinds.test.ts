import assert from "node:assert/strict";
import { test } from "node:test";

import { KINDS, kindIcon } from "./kinds.ts";

test("each kind has its codicon; a function inside a class is a method", () => {
  assert.equal(kindIcon("folder", false), "folder");
  assert.equal(kindIcon("file", false), "file-code");
  assert.equal(kindIcon("class", false), "symbol-class");
  assert.equal(kindIcon("function", false), "symbol-function");
  assert.equal(kindIcon("function", true), "symbol-method");
  assert.equal(kindIcon("class", true), "symbol-class", "a nested class is still a class");
});

test("no kind is drawn with the call icon: that is the arrow in front of the sites only", () => {
  for (const kind of Object.keys(KINDS) as (keyof typeof KINDS)[]) for (const inClass of [false, true]) assert.notEqual(kindIcon(kind, inClass), "call-outgoing");
});
