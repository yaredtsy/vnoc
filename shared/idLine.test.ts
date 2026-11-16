import assert from "node:assert/strict";
import test from "node:test";

import { dropsId, idValues } from "./idLine.ts";

// docs/17-edit-in-canvas.md §7: does saving this text take the function's ID away. The reader is the core's (`ids::read`), ported: the cases below are its own.

const ID_A = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";
const ID_B = "1Q8V5W4T2F7KX9M3B6D1RZ4H7K";
const withId = (id: string) => `def f():\n    """Does it.\n\n    ID: ${id}\n    """\n    return 1`;

const table: { name: string; base: string; written: string; drops: boolean }[] = [
  { name: "the ID line removed", base: withId(ID_A), written: 'def f():\n    """Does it."""\n    return 1', drops: true },
  { name: "the ID changed", base: withId(ID_A), written: withId(ID_B), drops: true },
  { name: "the ID kept, the body edited", base: withId(ID_A), written: withId(ID_A).replace("return 1", "return 2"), drops: false },
  { name: "the ID moved to another line of the docstring", base: withId(ID_A), written: `def f():\n    """Does it.\n    ID: ${ID_A}\n\n    More text.\n    """\n    return 1`, drops: false },
  { name: "the ID moved into the one-line docstring", base: withId(ID_A), written: `def f():\n    """Does it. ID: ${ID_A}"""\n    return 1`, drops: false },
  { name: "a function with no ID", base: "def f():\n    return 1", written: "def f():\n    return 2", drops: false },
  { name: "a function with no ID that gets one: nothing is lost", base: "def f():\n    return 1", written: withId(ID_A), drops: false },
  { name: "the ID kept at another indent (the host re-indents): the words are what count", base: withId(ID_A), written: withId(ID_A).replace("    ID:", "        ID:"), drops: false },
  { name: "`ID:` with no space before the value still counts, as for the core", base: withId(ID_A), written: withId(ID_A).replace("ID: ", "ID:"), drops: false },
  { name: "a label that is not the whole word (`KeyID:`) is no ID line", base: withId(ID_A), written: withId(ID_A).replace("ID:", "KeyID:"), drops: true },
  { name: "the ID line emptied: no value, no ID", base: withId(ID_A), written: withId(ID_A).replace(ID_A, ""), drops: true },
  { name: "a class: the method's ID taken away counts too", base: `class C:\n    """C.\n\n    ID: ${ID_A}\n    """\n\n    def m(self):\n        """M.\n\n        ID: ${ID_B}\n        """\n`, written: `class C:\n    """C.\n\n    ID: ${ID_A}\n    """\n\n    def m(self):\n        """M."""\n`, drops: true },
  { name: "a class: the class's own ID taken away, the method's kept", base: `class C:\n    """C.\n\n    ID: ${ID_A}\n    """\n\n    def m(self):\n        """M.\n\n        ID: ${ID_B}\n        """\n`, written: `class C:\n    """C."""\n\n    def m(self):\n        """M.\n\n        ID: ${ID_B}\n        """\n`, drops: true },
  { name: "the same text", base: withId(ID_A), written: withId(ID_A), drops: false },
];
for (const r of table) test(`drops the ID: ${r.name}`, () => assert.equal(dropsId(r.base, r.written), r.drops));

test("a docstring's quotes are glued to the words in source text: they are taken off, and an ID has none", () => {
  assert.deepEqual(idValues(`"""Summary. ID: ${ID_A}"""`), [ID_A], "closing quotes");
  assert.deepEqual(idValues(`"""ID: ${ID_A}"""`), [ID_A], "opening and closing quotes");
  assert.deepEqual(idValues(`r"""ID: ${ID_A}\n"""`), [ID_A], "a raw string");
  assert.deepEqual(idValues(`'''ID: ${ID_A}'''`), [ID_A], "single quotes");
  assert.deepEqual(idValues(`x = "ID: ${ID_A}"`), [ID_A], "any string that says it: the core reads prose the same way");
});

test("idValues: the value after `ID:`, every one, in order; a value with a colon or none is no ID (the core's own cases)", () => {
  assert.deepEqual(idValues("ID: abc"), ["abc"]);
  assert.deepEqual(idValues("ID:abc"), ["abc"]);
  assert.deepEqual(idValues("Summary.\n\n    ID: abc\n    "), ["abc"]);
  assert.deepEqual(idValues("ID: a\nID: b"), ["a", "b"]);
  assert.deepEqual(idValues("ID:"), []);
  assert.deepEqual(idValues("ID:   "), []);
  assert.deepEqual(idValues("ID: a:b"), []);
  assert.deepEqual(idValues("no marker here"), []);
  assert.deepEqual(idValues("KeyID: x"), []);
  assert.deepEqual(idValues("The text says ID: nothing-real, and nothing real below."), ["nothing-real,"], "prose that mentions the label counts, as for the core");
  assert.deepEqual(idValues("x = {'a': 1}  # not an ID"), []);
});
