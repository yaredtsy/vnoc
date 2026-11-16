import assert from "node:assert/strict";
import { test } from "node:test";

import { CODE_FIELD_CLASS, TEXT_FIELD } from "./codeField.ts";

// 9.2c: the code's container is a text field to React Flow and to the canvas's own keys, by one class.

test("the code's container is not dragged, panned, wheeled or keyed by the canvas", () => {
  const classes = CODE_FIELD_CLASS.split(" ");
  for (const c of ["nodrag", "nopan", "nowheel", "nokey"]) assert.ok(classes.includes(c), `the container has ${c}`);
});

test("the canvas's own keys leave a key alone in an input, a select, a textarea, a contenteditable and anything inside `nokey`", () => {
  const parts = TEXT_FIELD.split(",").map((s) => s.trim());
  for (const p of ["input", "select", "textarea", "[contenteditable]"]) assert.ok(parts.includes(p), p);
  assert.ok(parts.includes(".nokey"), "the class React Flow honours, on the code's container");
});

test("the one rule: the class the container carries is the class the selector names", () => {
  const nokey = CODE_FIELD_CLASS.split(" ").find((c) => c === "nokey");
  assert.ok(nokey && TEXT_FIELD.split(",").map((s) => s.trim()).includes(`.${nokey}`));
});
