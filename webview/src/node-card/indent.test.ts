import assert from "node:assert/strict";
import { test } from "node:test";

import { dedent, readingPrefix, reindent } from "./indent.ts";

// docs/17-edit-in-canvas.md §11 a. What is pinned: the pair gives back the same bytes when every line after the first starts with the prefix (empty lines
// included). The cases at the end are the ones it does not (the gap in §6); they are here so the loss is written down in code, not found later.

const SPACES = "    ";
const round: { name: string; prefix: string; text: string }[] = [
  { name: "spaces", prefix: SPACES, text: "def f(x):\n    return x + 1\n" },
  { name: "tabs", prefix: "\t", text: "def f(x):\n\tif x:\n\t\treturn 1\n\treturn 2" },
  { name: "empty lines", prefix: SPACES, text: "def f():\n    a = 1\n\n\n    return a" },
  { name: "a one-line function", prefix: SPACES, text: "def f(): return 1" },
  { name: "a function at the start of its line: no prefix", prefix: "", text: "def f():\n    return 1" },
  { name: "a docstring indented as the body", prefix: SPACES, text: 'def f():\n    """Does it.\n\n    More.\n    """\n    return 1' },
  { name: "a method, two levels in", prefix: "        ", text: "def m(self):\n        for x in self.xs:\n            yield x" },
  { name: "a trailing newline", prefix: SPACES, text: "def f():\n    return 1\n" },
];
for (const c of round) {
  test(`reindent(dedent(x)) is x: ${c.name}`, () => assert.equal(reindent(dedent(c.text, c.prefix), c.prefix), c.text));
}

test("dedent takes the prefix off every line after the first, and not the first", () => {
  assert.equal(dedent("def f():\n    return 1\n\n    pass", SPACES), "def f():\nreturn 1\n\npass");
  assert.equal(dedent("def f():\n\t\treturn 1", "\t"), "def f():\n\treturn 1", "one level of the prefix, not all the whitespace");
});

test("dedent takes off only what is there: a line with less indent than the prefix loses all of it, and no more", () => {
  assert.equal(dedent('def f():\n    """doc\n  less\n    """', SPACES), 'def f():\n"""doc\nless\n"""');
  assert.equal(dedent("def f():\n  x = 1\n    y = 2", SPACES), "def f():\nx = 1\ny = 2");
});

test("reindent gives the prefix to every line after the first that is not empty, and not to the first", () => {
  assert.equal(reindent("def f():\nreturn 1\n\npass", SPACES), "def f():\n    return 1\n\n    pass");
  assert.equal(reindent("x", SPACES), "x");
});

test("the reading path's prefix is as many spaces as the range's column", () => {
  assert.equal(readingPrefix(0), "");
  assert.equal(readingPrefix(4), SPACES);
  assert.equal(dedent("def m(self):\n        return 1", readingPrefix(4)), "def m(self):\n    return 1", "what the card showed before, byte for byte");
});

// the gap of 17 §6: the pair is not an inverse for these. Each line says what comes back instead.
test("KNOWN LOSS (§6 gap): a line indented less than the prefix comes back with the whole prefix", () => {
  const text = 'def f():\n    """doc\n  less\n    """';
  assert.equal(reindent(dedent(text, SPACES), SPACES), 'def f():\n    """doc\n    less\n    """', "2 spaces became 4");
});

test("KNOWN LOSS (§6 gap): a line of only whitespace comes back empty", () => {
  assert.equal(reindent(dedent("def f():\n    a = 1\n    \n    return a", SPACES), SPACES), "def f():\n    a = 1\n\n    return a");
});

test("KNOWN LOSS (§6 gap): a blank line of a CRLF file loses its carriage return, and a blank one that keeps it gets the prefix", () => {
  assert.equal(dedent("def f():\r\n    a = 1\r\n\r\n    return a", SPACES), "def f():\r\na = 1\r\n\nreturn a");
  assert.equal(reindent("def f():\r\na = 1\r\n\r\nreturn a", SPACES), "def f():\r\n    a = 1\r\n    \r\n    return a");
});
