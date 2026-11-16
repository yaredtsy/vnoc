import assert from "node:assert/strict";
import { test } from "node:test";

import { dirtyPaths, nextDirtyFiles, type OpenDocument } from "./dirtyFiles.ts";

// Which files have unsaved changes (docs/17-edit-in-canvas.md step 9.1). Run by `pnpm --filter vnoc unit`.

const FOLDER = "/work/app";
const doc = (fsPath: string, isDirty: boolean, isFile = true): OpenDocument => ({ fsPath, isDirty, isFile });

test("only dirty files under the folder: relative, with /, sorted", () => {
  const docs = [doc("/work/app/pkg/b.py", true), doc("/work/app/a.py", true), doc("/work/app/clean.py", false)];
  assert.deepEqual(dirtyPaths(docs, FOLDER), ["a.py", "pkg/b.py"]);
});

test("a file outside the folder is not in it, nor one in a folder that only starts with the same letters", () => {
  const docs = [doc("/elsewhere/a.py", true), doc("/work/app2/a.py", true), doc("/work/app", true)];
  assert.deepEqual(dirtyPaths(docs, FOLDER), []);
});

test("untitled documents and anything that is not a file are not in it", () => {
  assert.deepEqual(dirtyPaths([doc("/work/app/Untitled-1", true, false), doc("/work/app/out.log", true, false)], FOLDER), []);
});

test("two documents of one file count once", () => {
  assert.deepEqual(dirtyPaths([doc("/work/app/a.py", true), doc("/work/app/a.py", true)], FOLDER), ["a.py"]);
});

const changes: { name: string; previous: string[]; docs: OpenDocument[]; expect: string[] | null }[] = [
  { name: "nothing was dirty, nothing is", previous: [], docs: [doc("/work/app/a.py", false)], expect: null },
  { name: "a file became dirty", previous: [], docs: [doc("/work/app/a.py", true)], expect: ["a.py"] },
  { name: "a keystroke in a file that was dirty: the same set", previous: ["a.py"], docs: [doc("/work/app/a.py", true)], expect: null },
  { name: "saved, or reverted, or undone back to the saved text", previous: ["a.py"], docs: [doc("/work/app/a.py", false)], expect: [] },
  { name: "closed without saving: the document is gone", previous: ["a.py"], docs: [], expect: [] },
  { name: "a second file became dirty", previous: ["a.py"], docs: [doc("/work/app/a.py", true), doc("/work/app/b.py", true)], expect: ["a.py", "b.py"] },
  { name: "one of two was saved", previous: ["a.py", "b.py"], docs: [doc("/work/app/a.py", false), doc("/work/app/b.py", true)], expect: ["b.py"] },
  { name: "the order the documents come in does not matter", previous: ["a.py", "b.py"], docs: [doc("/work/app/b.py", true), doc("/work/app/a.py", true)], expect: null },
  { name: "an untitled document becoming dirty is no change", previous: [], docs: [doc("/work/app/Untitled-1", true, false)], expect: null },
];
for (const c of changes) {
  test(`the set changed or not: ${c.name}`, () => assert.deepEqual(nextDirtyFiles(c.previous, c.docs, FOLDER), c.expect));
}
