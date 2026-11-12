import assert from "node:assert/strict";
import { test } from "node:test";

import type { CallMark, CallTreeNode, CallTreeResult, Range } from "@vnoc/shared/messages.ts";
import { callLabel, callRows, isStale, listedSites, siteLabel, siteText } from "./calls.ts";

const at = (line: number): Range => ({ start: { line, character: 4 }, end: { line, character: 7 } });
const siteAt = <P extends string | null>(line: number, path: P): Range & { path: P } => ({ ...at(line), path });
const node = (name: string, over: Partial<CallTreeNode> = {}, children: CallTreeNode[] = []): CallTreeNode => ({
  key: `${name}.py::${name}`,
  name,
  qualname: name,
  path: `${name}.py`,
  kind: "function",
  call_count: 1,
  call_sites: [siteAt(1, "app.py")],
  marks: [],
  children,
  ...over,
});

// main ├ b ×2 ├ c        ├ d (recursive)
//      └ e (a library: no key, no path)
const tree = node("main", { path: "app.py", call_sites: [] }, [node("b", { call_count: 2, call_sites: [siteAt(3, "app.py"), siteAt(5, "app.py")] }, [node("c")]), node("d", { marks: ["recursive"] }), node("e", { key: null, path: null })]);

test("one row for each node under the entry, parents before children, with their depth", () => {
  const rows = callRows(tree);
  assert.deepEqual(rows.map((r) => [r.node.name, r.depth]), [["b", 0], ["c", 1], ["d", 0], ["e", 0]]);
  assert.equal(rows.length, 4, "the entry itself is the card, not a row");
});

test("row ids are unique, so a click and an open list always find one row", () => {
  const rows = callRows(tree);
  assert.deepEqual(rows.map((r) => r.id), ["0", "0.0", "1", "2"]);
  assert.equal(new Set(rows.map((r) => r.id)).size, rows.length);
});

test("a node called more than once says how many times, and a single call says nothing", () => {
  assert.equal(callLabel(node("b", { call_count: 2 })), "b ×2");
  assert.equal(callLabel(node("b", { call_count: 1 })), "b");
  assert.deepEqual(callRows(tree).map((r) => r.label), ["b ×2", "c", "d", "e"]);
});

test("a mark is a badge with its own words", () => {
  const marks = (m: CallMark[]) => callRows(node("x", {}, [node("y", { marks: m })]))[0].marks.map((b) => b.text);
  assert.deepEqual(marks(["recursive"]), ["↻ recursive"]);
  assert.deepEqual(marks(["cut"]), ["✂ cut"]);
  assert.deepEqual(marks(["incomplete"]), ["… incomplete"]);
  assert.deepEqual(marks(["recursive", "cut"]), ["↻ recursive", "✂ cut"]);
  assert.deepEqual(marks([]), []);
});

test("a row's call sites are in its parent's file", () => {
  const rows = callRows(tree);
  assert.equal(rows[0].parentPath, "app.py", "b is called from the entry's file");
  assert.equal(rows[1].parentPath, "b.py", "c is called from b's file");
});

test("a node the project does not know has no key, and nothing to select", () => {
  const library = callRows(tree).find((r) => r.node.name === "e")!;
  assert.equal(library.node.key, null);
  assert.equal(callRows(tree).filter((r) => r.node.key !== null).length, 3);
});

test("a call site is named by its line, counted from 1", () => {
  assert.equal(siteLabel(at(0)), "line 1");
  assert.equal(siteLabel(at(11)), "line 12");
});

test("a tree is stale when the index has moved on since its keys were mapped", () => {
  const result = (revision: number): CallTreeResult => ({ revision, root: tree });
  assert.equal(isStale(result(41), 41), false);
  assert.equal(isStale(result(41), 42), true);
  assert.equal(isStale(result(43), 42), false, "a tree can be newer than the structure the page holds for a moment");
});

test("an entry that calls nothing has no rows", () => {
  assert.deepEqual(callRows(node("main")), []);
});

test("a site in the parent's file is named by its line, in another file by the file's name and the line", () => {
  assert.equal(siteText(siteAt(11, "pkg/child.py"), "pkg/child.py"), "line 12");
  assert.equal(siteText(siteAt(11, "pkg/base.py"), "pkg/child.py"), "base.py:12");
  assert.equal(siteText(siteAt(0, "base.py"), "pkg/child.py"), "base.py:1", "a file at the top of the folder has no directory to cut");
});

test("a site with no file is not listed, because there is nowhere to open it", () => {
  const b = node("b", { call_sites: [siteAt(3, "app.py"), siteAt(4, null), siteAt(5, "lib/x.py")] });
  assert.deepEqual(listedSites(b).map((s) => s.start.line), [3, 5]);
  assert.deepEqual(listedSites(node("c", { call_sites: [siteAt(1, null)] })), [], "no site is left: no toggle");
});
