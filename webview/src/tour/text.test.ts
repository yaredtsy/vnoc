import assert from "node:assert/strict";
import { test } from "node:test";

import { pieces } from "./text.ts";

test("plain text is one piece, and an empty one is none", () => {
  assert.deepEqual(pieces("It checks the stock."), [{ kind: "text", text: "It checks the stock." }]);
  assert.deepEqual(pieces(""), []);
});

test("`x` becomes a code span, and *y* emphasis", () => {
  assert.deepEqual(pieces("reads `quantity` units"), [{ kind: "text", text: "reads " }, { kind: "code", text: "quantity" }, { kind: "text", text: " units" }]);
  assert.deepEqual(pieces("it *never* changes it"), [{ kind: "text", text: "it " }, { kind: "em", text: "never" }, { kind: "text", text: " changes it" }]);
  assert.deepEqual(pieces("`a` and *b*"), [{ kind: "code", text: "a" }, { kind: "text", text: " and " }, { kind: "em", text: "b" }]);
});

test("a <b> stays text: nothing is HTML, and nothing else is a mark", () => {
  assert.deepEqual(pieces("a <b>bold</b> <script>x</script>"), [{ kind: "text", text: "a <b>bold</b> <script>x</script>" }]);
  assert.deepEqual(pieces("# heading\n- item\n[link](http://x)\n![img](y)"), [{ kind: "text", text: "# heading\n- item\n[link](http://x)\n![img](y)" }]);
  assert.deepEqual(pieces("**strong** and __under__"), [{ kind: "text", text: "**strong** and __under__" }], "double marks are text");
  assert.deepEqual(pieces("2 * 3 * 4"), [{ kind: "text", text: "2 * 3 * 4" }], "a star with spaces is a star");
});

test("what is inside a code span is not emphasis, and an unclosed mark is text", () => {
  assert.deepEqual(pieces("`a *b* c`"), [{ kind: "code", text: "a *b* c" }]);
  assert.deepEqual(pieces("an `unclosed span and *an* emphasis"), [{ kind: "text", text: "an `unclosed span and " }, { kind: "em", text: "an" }, { kind: "text", text: " emphasis" }]);
  assert.deepEqual(pieces("one * alone"), [{ kind: "text", text: "one * alone" }]);
});

test("a code span with HTML in it is the characters", () => {
  assert.deepEqual(pieces("`<i>x</i>`"), [{ kind: "code", text: "<i>x</i>" }]);
});
