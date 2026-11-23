import assert from "node:assert/strict";
import { test } from "node:test";

import { languageOf } from "./languages.ts";

test("Bend code is coloured with Python's grammar, whatever the case of the extension", () => {
  assert.equal(languageOf("demos/app_win_is_bug_2d/PROOF.bend"), "python");
  assert.equal(languageOf("main.BEND"), "python");
});

test("the other languages keep their grammar; an unknown extension is plain text", () => {
  assert.equal(languageOf("httpx/_client.py"), "python");
  assert.equal(languageOf("src/lib.rs"), "rust");
  assert.equal(languageOf("README.md"), "markdown");
  assert.equal(languageOf("Makefile"), "plaintext");
  assert.equal(languageOf("a.hvm"), "plaintext");
});
