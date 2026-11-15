import assert from "node:assert/strict";
import { test } from "node:test";

import { type Contents, contentsText, metaOf } from "./metaOf.ts";

type In = Parameters<typeof metaOf>[0];
const none: Contents = { folders: 0, files: 0, classes: 0, functions: 0 };
const base: In = { id: "k", kind: "function", path: "app/core/models/conversation_state.py", line: 382, parentId: "app/core/models/conversation_state.py", syntaxError: false, contents: none };

const table: { name: string; node: Partial<In>; text: string; tip: string; error?: boolean }[] = [
  { name: "a function under its file", node: {}, text: "line 382", tip: "app/core/models/conversation_state.py:382" },
  { name: "a method under its class", node: { kind: "function", parentId: "k::Order" }, text: "line 382", tip: "app/core/models/conversation_state.py:382" },
  { name: "a class under its file", node: { kind: "class" }, text: "line 382", tip: "app/core/models/conversation_state.py:382" },
  { name: "a def that is the view's root: file name and line, the folders in the tooltip", node: { parentId: null }, text: "conversation_state.py:382", tip: "app/core/models/conversation_state.py:382" },
  { name: "a def with no range, as the root: just the file name", node: { parentId: null, line: null }, text: "conversation_state.py", tip: "app/core/models/conversation_state.py" },
  { name: "a file under its folder: its top-level defs", node: { kind: "file", line: null, parentId: "app/core/models", contents: { ...none, classes: 2, functions: 9 } }, text: "2 classes · 9 functions", tip: "app/core/models/conversation_state.py" },
  { name: "one of each says it in the singular", node: { kind: "file", line: null, parentId: "d", contents: { ...none, classes: 1, functions: 1 } }, text: "1 class · 1 function", tip: "app/core/models/conversation_state.py" },
  { name: "a file with only functions leaves the classes out", node: { kind: "file", line: null, parentId: "d", contents: { ...none, functions: 3 } }, text: "3 functions", tip: "app/core/models/conversation_state.py" },
  { name: "a file with nothing in it: empty", node: { kind: "file", line: null, parentId: "d" }, text: "empty", tip: "app/core/models/conversation_state.py" },
  { name: "a folder under its folder: its folders and files", node: { kind: "folder", path: "app/core", line: null, parentId: "app", contents: { ...none, folders: 3, files: 12 } }, text: "3 folders · 12 files", tip: "app/core/" },
  { name: "a folder with files only", node: { kind: "folder", path: "app/core", line: null, parentId: "app", contents: { ...none, files: 1 } }, text: "1 file", tip: "app/core/" },
  { name: "an empty folder: empty", node: { kind: "folder", path: "app/core", line: null, parentId: "app" }, text: "empty", tip: "app/core/" },
  { name: "a file that is the root of the view: its path (the box cuts it at the start)", node: { kind: "file", line: null, parentId: null }, text: "app/core/models/conversation_state.py", tip: "app/core/models/conversation_state.py" },
  { name: "a folder that is the root: its path and a slash", node: { kind: "folder", path: "app/core", line: null, parentId: null }, text: "app/core/", tip: "app/core/" },
  { name: "the opened folder says nothing", node: { id: "/", kind: "folder", path: "", line: null, parentId: null }, text: "", tip: "" },
  { name: "a file that did not parse: syntax error instead, the path still in the tooltip", node: { kind: "file", line: null, parentId: "d", syntaxError: true, contents: { ...none, functions: 2 } }, text: "syntax error", tip: "app/core/models/conversation_state.py", error: true },
  { name: "syntax error wins at the root too", node: { kind: "file", line: null, parentId: null, syntaxError: true }, text: "syntax error", tip: "app/core/models/conversation_state.py", error: true },
];
for (const row of table) {
  test(`metaOf: ${row.name}`, () => assert.deepEqual(metaOf({ ...base, ...row.node }), { text: row.text, tip: row.tip, error: row.error ?? false }));
}

test("contentsText: parts that are 0 are left out", () => {
  assert.equal(contentsText("folder", { ...none, folders: 2 }), "2 folders");
  assert.equal(contentsText("file", { ...none, classes: 2 }), "2 classes");
});
