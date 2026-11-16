import assert from "node:assert/strict";
import { test } from "node:test";

import { dedent, readingPrefix, reindent, splitLines } from "./indent.ts";

// docs/17-edit-in-canvas.md §11 a: the card shows `dedent(base)` and the host writes `reindent(draft, base, prefix, eol)`.

const SPACES = "    ";
const LF = "\n";
const CRLF = "\r\n";

// an unchanged draft gives back the same bytes, whatever the text: lines the user did not change are the base's own lines
const round: { name: string; prefix: string; eol: string; text: string }[] = [
  { name: "spaces", prefix: SPACES, eol: LF, text: "def f(x):\n    return x + 1\n" },
  { name: "tabs", prefix: "\t", eol: LF, text: "def f(x):\n\tif x:\n\t\treturn 1\n\treturn 2" },
  { name: "empty lines", prefix: SPACES, eol: LF, text: "def f():\n    a = 1\n\n\n    return a" },
  { name: "a line of only whitespace", prefix: SPACES, eol: LF, text: "def f():\n    a = 1\n    \n    return a" },
  { name: "trailing whitespace on a line", prefix: SPACES, eol: LF, text: "def f():\n    a = 1   \n    return a\t" },
  { name: "a docstring line at the margin", prefix: SPACES, eol: LF, text: 'def f():\n    """doc\nat the margin\n    """\n    return 1' },
  { name: "a docstring line indented less than the def", prefix: SPACES, eol: LF, text: 'def f():\n    """doc\n  less\n    """' },
  { name: "a docstring indented as the body", prefix: SPACES, eol: LF, text: 'def f():\n    """Does it.\n\n    More.\n    """\n    return 1' },
  { name: "a CRLF file with blank lines", prefix: SPACES, eol: CRLF, text: "def f():\r\n    a = 1\r\n\r\n    return a\r\n" },
  { name: "a CRLF file with a whitespace-only line", prefix: SPACES, eol: CRLF, text: "def f():\r\n    a = 1\r\n    \r\n    return a" },
  { name: "a one-line function", prefix: SPACES, eol: LF, text: "def f(): return 1" },
  { name: "no prefix: the definition starts its line", prefix: "", eol: LF, text: "def f():\n    return 1" },
  { name: "a method, two levels in", prefix: "        ", eol: LF, text: "def m(self):\n        for x in self.xs:\n            yield x" },
  { name: "a trailing newline", prefix: SPACES, eol: LF, text: "def f():\n    return 1\n" },
  { name: "mixed indent, tabs under spaces", prefix: SPACES, eol: LF, text: "def f():\n    if x:\n\treturn 1\n  return 2" },
];
for (const c of round) {
  test(`reindent(dedent(x), x) is x: ${c.name}`, () => assert.equal(reindent(dedent(c.text, c.prefix), c.text, c.prefix, c.eol), c.text));
}

const BASE = 'def method(self):\n        """Does it.\nat the margin\n        """\n        a = 1   \n\n        b = 2\n        return a + b';
const P = "    ".repeat(2);
const shown = dedent(BASE, P);

test("an edit in the middle keeps the bytes of every other line: the trailing whitespace, the margin line, the blank line", () => {
  const draft = shown.replace("b = 2", "b = 3");
  assert.equal(reindent(draft, BASE, P, LF), BASE.replace("b = 2", "b = 3"));
});

test("a changed line gets the prefix, an empty line stays empty, and the first line goes in as it is", () => {
  assert.equal(reindent(shown.replace("def method(self):", "def method(self, x):"), BASE, P, LF), BASE.replace("def method(self):", "def method(self, x):"));
  assert.equal(reindent("def f():\n\nx = 1\n\ny = 2", "def f():\n        pass", P, LF), "def f():\n\n        x = 1\n\n        y = 2");
});

test("an added line gets the prefix and the file's line ending, in a CRLF file too", () => {
  const base = "def f():\r\n    a = 1\r\n    return a";
  const draft = dedent(base, SPACES).replace("a = 1", "a = 1\nb = 2");
  assert.equal(reindent(draft, base, SPACES, CRLF), "def f():\r\n    a = 1\r\n    b = 2\r\n    return a");
  assert.equal(reindent(draft.replace(/\r?\n/g, "\r\n"), base, SPACES, CRLF), "def f():\r\n    a = 1\r\n    b = 2\r\n    return a", "the card's own line endings do not matter");
});

test("a line added at the top of the body, and one at the end", () => {
  assert.equal(reindent("def f():\nfirst = 1\nreturn 1\nlast = 2", "def f():\n    return 1", SPACES, LF), "def f():\n    first = 1\n    return 1\n    last = 2");
});

test("a deleted line is gone, and nothing else moves", () => {
  assert.equal(reindent(shown.replace("\n\nb = 2", ""), BASE, P, LF), BASE.replace("\n\n        b = 2", ""));
  assert.equal(reindent("def f():", "def f():\n    a\n    b", SPACES, LF), "def f():");
});

test("a line moved elsewhere in the draft is the base's own line wherever it is matched", () => {
  const base = "def f():\n    a = 1\n  margin\n    b = 2";
  assert.equal(reindent("def f():\nb = 2\na = 1\nmargin", base, SPACES, LF), "def f():\n    b = 2\n    a = 1\n  margin", "the matched ones keep their bytes, the rest are new lines");
});

test("the leftover: an edited margin line gains the prefix", () => {
  assert.equal(reindent(shown.replace("at the margin", "at the new margin"), BASE, P, LF), BASE.replace("at the margin", "        at the new margin"));
});

test("the first line of the draft is not matched to a line below it, and the base's first line is not put in a body", () => {
  assert.equal(reindent("x = 1\ndef f():\nreturn 1", "def f():\n    return 1", SPACES, LF), "x = 1\n    def f():\n    return 1");
});

test("a line ending that differs from the document's is not given back: the one thing", () => {
  assert.equal(reindent("def f():\na\nb", "def f():\n    a\r\n    b", SPACES, LF), "def f():\n    a\n    b");
});

test("splitLines takes both line endings", () => assert.deepEqual(splitLines("a\r\nb\nc\r\n"), ["a", "b", "c", ""]));

// the diff is cheap when the edit is local
function bigClass(lines: number): string {
  const body = Array.from({ length: lines }, (_, i) => (i % 17 === 0 ? "" : `        x${i} = ${i}  `));
  return ["class Big:", ...body].join("\n");
}

test("a 3,000-line class with one changed line is written in well under 50 ms, and only that line differs", () => {
  const base = bigClass(3000);
  const draft = dedent(base, "    ").split("\n");
  draft[1500] = draft[1500].replace("x1499", "changed");
  const t = performance.now();
  const out = reindent(draft.join("\n"), base, "    ", LF);
  const ms = performance.now() - t;
  assert.ok(ms < 50, `${ms.toFixed(1)} ms`);
  const was = base.split("\n");
  const now = out.split("\n");
  assert.equal(now.length, was.length);
  assert.deepEqual(now.map((l, i) => (l === was[i] ? null : i)).filter((i) => i !== null).length, 1);
});

test("a rewrite too big to diff is written as new text: every line gets the prefix, and nothing breaks", () => {
  const base = ["def f():", ...Array.from({ length: 2100 }, (_, i) => `    old${i}`)].join("\n");
  const draft = ["def f():", ...Array.from({ length: 2100 }, (_, i) => `new${i}`)].join("\n");
  const out = reindent(draft, base, SPACES, LF).split("\n");
  assert.equal(out[1], "    new0");
  assert.equal(out.length, 2101);
});

// dedent, for display
test("dedent takes the prefix off every line after the first, and not the first", () => {
  assert.equal(dedent("def f():\n    return 1\n\n    pass", SPACES), "def f():\nreturn 1\n\npass");
  assert.equal(dedent("def f():\n\t\treturn 1", "\t"), "def f():\n\treturn 1", "one level of the prefix, not all the whitespace");
});

test("dedent takes off only what is there: a line with less indent than the prefix loses all of it, and no more", () => {
  assert.equal(dedent('def f():\n    """doc\n  less\n    """', SPACES), 'def f():\n"""doc\nless\n"""');
  assert.equal(dedent("def f():\n  x = 1\n    y = 2", SPACES), "def f():\nx = 1\ny = 2");
});

test("the reading path's prefix is as many spaces as the range's column", () => {
  assert.equal(readingPrefix(0), "");
  assert.equal(readingPrefix(4), SPACES);
  assert.equal(dedent("def m(self):\n        return 1", readingPrefix(4)), "def m(self):\n    return 1", "what the card showed before, byte for byte");
});
