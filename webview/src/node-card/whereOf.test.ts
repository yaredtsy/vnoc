import assert from "node:assert/strict";
import { test } from "node:test";

import { whereOf } from "./whereOf.ts";

test("the opened folder says nothing", () => assert.equal(whereOf({ id: "/", kind: "folder", path: "", line: null }), ""));
test("a definition: path:line", () => assert.equal(whereOf({ id: "f", kind: "function", path: "app/api.py", line: 11 }), "app/api.py:11"));
test("a file: its path", () => assert.equal(whereOf({ id: "a", kind: "file", path: "app/api.py", line: null }), "app/api.py"));
test("a folder: its path and a slash", () => assert.equal(whereOf({ id: "app", kind: "folder", path: "app", line: null }), "app/"));
