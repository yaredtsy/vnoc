import assert from "node:assert/strict";
import * as path from "node:path";
import { test } from "node:test";

import { dirtyPaths, nextDirtyFiles, type OpenDocument } from "./dirtyFiles.ts";

// Which files have unsaved changes (docs/17-edit-in-canvas.md step 9.1). Run by `pnpm --filter vnoc unit`.
// Each test runs once per platform's path rules, so the Windows ones are tested on every machine. The answers keep `/` on both: the pages compare them with the core's paths.

const doc = (fsPath: string, isDirty: boolean, isFile = true): OpenDocument => ({ fsPath, isDirty, isFile });

for (const [name, p, FOLDER] of [
  ["posix", path.posix, "/work/app"],
  ["win32", path.win32, "C:\\work\\app"],
] as const) {
  const inside = (...parts: string[]) => p.join(FOLDER, ...parts);

  test(`${name}: only dirty files under the folder: relative, with /, sorted`, () => {
    const docs = [doc(inside("pkg", "b.py"), true), doc(inside("a.py"), true), doc(inside("clean.py"), false)];
    assert.deepEqual(dirtyPaths(docs, FOLDER, p), ["a.py", "pkg/b.py"]);
  });

  test(`${name}: a file outside the folder is not in it, nor one in a folder that only starts with the same letters`, () => {
    const docs = [
      doc(p.join(p.dirname(FOLDER), "..", "elsewhere", "a.py"), true),
      doc(FOLDER + "2" + p.sep + "a.py", true),
      doc(FOLDER, true),
    ];
    assert.deepEqual(dirtyPaths(docs, FOLDER, p), []);
  });

  test(`${name}: untitled documents and anything that is not a file are not in it`, () => {
    assert.deepEqual(dirtyPaths([doc(inside("Untitled-1"), true, false), doc(inside("out.log"), true, false)], FOLDER, p), []);
  });

  test(`${name}: two documents of one file count once`, () => {
    assert.deepEqual(dirtyPaths([doc(inside("a.py"), true), doc(inside("a.py"), true)], FOLDER, p), ["a.py"]);
  });

  const changes: { name: string; previous: string[]; docs: OpenDocument[]; expect: string[] | null }[] = [
    { name: "nothing was dirty, nothing is", previous: [], docs: [doc(inside("a.py"), false)], expect: null },
    { name: "a file became dirty", previous: [], docs: [doc(inside("a.py"), true)], expect: ["a.py"] },
    { name: "a keystroke in a file that was dirty: the same set", previous: ["a.py"], docs: [doc(inside("a.py"), true)], expect: null },
    { name: "saved, or reverted, or undone back to the saved text", previous: ["a.py"], docs: [doc(inside("a.py"), false)], expect: [] },
    { name: "closed without saving: the document is gone", previous: ["a.py"], docs: [], expect: [] },
    { name: "a second file became dirty", previous: ["a.py"], docs: [doc(inside("a.py"), true), doc(inside("b.py"), true)], expect: ["a.py", "b.py"] },
    { name: "one of two was saved", previous: ["a.py", "b.py"], docs: [doc(inside("a.py"), false), doc(inside("b.py"), true)], expect: ["b.py"] },
    { name: "the order the documents come in does not matter", previous: ["a.py", "b.py"], docs: [doc(inside("b.py"), true), doc(inside("a.py"), true)], expect: null },
    { name: "an untitled document becoming dirty is no change", previous: [], docs: [doc(inside("Untitled-1"), true, false)], expect: null },
  ];
  for (const c of changes) {
    test(`${name}: the set changed or not: ${c.name}`, () => assert.deepEqual(nextDirtyFiles(c.previous, c.docs, FOLDER, p), c.expect));
  }
}

test("win32: a dirty file in a subfolder is answered with /, no backslash", () => {
  assert.deepEqual(dirtyPaths([doc("C:\\work\\app\\pkg\\b.py", true)], "C:\\work\\app", path.win32), ["pkg/b.py"]);
});
