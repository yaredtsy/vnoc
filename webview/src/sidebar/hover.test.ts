import assert from "node:assert/strict";
import { test } from "node:test";

import type { Kind } from "@vnoc/shared/messages.ts";
import { hoverParts } from "./hover.ts";

const RANGE = { start: { line: 10, character: 0 }, end: { line: 14, character: 0 } };
const node = (kind: Kind, over: object = {}) => ({ name: "checkout", kind, path: "app/api.py", range: null, docs: 0, link: null as "marker" | "name" | null, ...over });

// the table of 07 §5: kind × docs × link
const table: { name: string; node: ReturnType<typeof node>; expect: ReturnType<typeof hoverParts> }[] = [
  { name: "a function: its file and line (1-based), no docs", node: node("function", { range: RANGE }), expect: { name: "checkout", kind: "function", where: "app/api.py:11", docs: null, link: null } },
  { name: "a function with one doc, its ID in the code", node: node("function", { range: RANGE, docs: 1, link: "marker" }), expect: { name: "checkout", kind: "function", where: "app/api.py:11", docs: "1 doc", link: "ID in the code" } },
  { name: "a class with docs found by name only", node: node("class", { name: "Order", range: RANGE, docs: 2, link: "name" }), expect: { name: "Order", kind: "class", where: "app/api.py:11", docs: "2 docs", link: "found by name only" } },
  { name: "a file has no line: its path", node: node("file", { name: "api.py" }), expect: { name: "api.py", kind: "file", where: "app/api.py", docs: null, link: null } },
  { name: "a folder: its path", node: node("folder", { name: "app", path: "app", docs: 3, link: "marker" }), expect: { name: "app", kind: "folder", where: "app", docs: "3 docs", link: "ID in the code" } },
  { name: "no docs: no word about how they were found", node: node("function", { range: RANGE, docs: 0, link: "marker" }), expect: { name: "checkout", kind: "function", where: "app/api.py:11", docs: null, link: null } },
];
for (const t of table) test(`hover card: ${t.name}`, () => assert.deepEqual(hoverParts(t.node), t.expect));
