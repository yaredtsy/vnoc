import assert from "node:assert/strict";
import { test } from "node:test";

import { callExists, callId, callLookup, callNodesIn, isCallId, isStale, listedSites, parseCallId, resolveCall, siteLabel, siteText, stepOf } from "./calls.ts";
import type { CallTreeNode, CallTreeResult, Range, TreeNode } from "./messages";

const at = (line: number): Range => ({ start: { line, character: 4 }, end: { line, character: 7 } });
const siteAt = <P extends string | null>(line: number, path: P): Range & { path: P } => ({ ...at(line), path });
const call = (name: string, over: Partial<CallTreeNode> = {}, children: CallTreeNode[] = []): CallTreeNode => ({
  key: `${name}.py::${name}`, name, qualname: name, path: `${name}.py`, kind: "function", call_count: 1, call_sites: [siteAt(1, "app.py")], marks: [], children, ...over,
});

// checkout ├ order ├ reserve ├ check     └ lib (the project does not know it)
//          └ order2 …
const tree = call("checkout", { call_sites: [] }, [call("order", {}, [call("reserve", {}, [call("check")]), call("check")]), call("lib", { key: null, path: "lib/x.py", qualname: "lib" })]);
const OWNER = "checkout.py::checkout";

test("a call id is the JSON of the owner and the steps, and reads back", () => {
  const id = callId(OWNER, ["order.py::order", "reserve.py::reserve"]);
  assert.equal(id, `call:["checkout.py::checkout","order.py::order","reserve.py::reserve"]`);
  assert.deepEqual(parseCallId(id), { owner: OWNER, steps: ["order.py::order", "reserve.py::reserve"] });
  assert.equal(isCallId(id), true);
  assert.equal(isCallId("app/api.py::checkout"), false);
});

// the characters a path or a name may hold: the id must read back whole, and be one id
const odd = ["a::b", "dir/with space/x.py::f", "x#2", "a>b", `q"uote`, "back\\slash", "ünï/code.py::fn", "call:inside"];
for (const key of odd) {
  test(`JSON survives ${JSON.stringify(key)} in a path`, () => {
    const id = callId(key, [key, `${key}#2`]);
    assert.deepEqual(parseCallId(id), { owner: key, steps: [key, `${key}#2`] });
  });
}

test("what is not a call id does not parse", () => {
  for (const bad of ["", "call:", "call:{}", "call:[]", "call:[1]", "call:not json", "app/api.py", `call:["a",2]`]) assert.equal(parseCallId(bad), null, bad);
});

test("a step is the callee's key; a callee the project does not know is ? + path + :: + qualname", () => {
  const children = tree.children;
  assert.equal(stepOf(children[0], children), "order.py::order");
  assert.equal(stepOf(children[1], children), "?lib/x.py::lib");
  assert.equal(stepOf(call("n", { key: null, path: null, qualname: "n" }), []), "?::n");
});

test("twin steps: the second gets #2 and the third #3", () => {
  const twins = [call("a"), call("b"), call("a"), call("a")];
  assert.deepEqual(twins.map((n) => stepOf(n, twins)), ["a.py::a", "b.py::b", "a.py::a#2", "a.py::a#3"]);
});

test("ids are stable when a call is inserted above: they are keys, not positions", () => {
  const before = callNodesIn(OWNER, tree);
  const inserted = call("checkout", { call_sites: [] }, [call("brand_new"), ...tree.children]);
  const after = callNodesIn(OWNER, inserted);
  for (const { id, node } of before) assert.equal(after.find((a) => a.node === node)?.id, id, `${node.name} kept its id`);
  assert.equal(after.length, before.length + 1);
});

test("every node under the entry has one id, and the ids resolve to their nodes", () => {
  const nodes = callNodesIn(OWNER, tree);
  assert.deepEqual(nodes.map((n) => n.node.name), ["order", "reserve", "check", "check", "lib"]);
  assert.equal(new Set(nodes.map((n) => n.id)).size, nodes.length, "no two the same");
  for (const { id, node } of nodes) assert.equal(resolveCall({ [OWNER]: { root: tree } }, id), node);
});

test("a recursive call (A → B → A): the ids stay distinct and finite", () => {
  const a = "a.py::a";
  const recursive = call("a", { call_sites: [] }, [call("b", {}, [call("a", { marks: ["recursive"] })])]);
  const nodes = callNodesIn(a, recursive);
  assert.equal(nodes.length, 2, "the walk ends where the tree ends: the recursive node is shown, not entered");
  assert.deepEqual(nodes.map((n) => parseCallId(n.id)!.steps), [["b.py::b"], ["b.py::b", "a.py::a"]]);
  assert.notEqual(nodes[1].id, callId(a, []), "the A below B is not the owner A");
  assert.equal(new Set(nodes.map((n) => n.id)).size, 2);
});

test("resolveCall: a path that is not in the tree, an owner we do not hold, and the owner alone give nothing", () => {
  const trees = { [OWNER]: { root: tree } };
  assert.equal(resolveCall(trees, callId(OWNER, ["order.py::order", "nope"])), undefined);
  assert.equal(resolveCall(trees, callId("other", ["x"])), undefined);
  assert.equal(resolveCall(trees, callId(OWNER, [])), undefined, "the owner is the card, not a call node");
  assert.equal(resolveCall(trees, "app/api.py"), undefined);
});

test("the lookup names the callee, key null for one the project does not know, and nothing for a tree we do not hold", () => {
  const lookup = callLookup({ [OWNER]: { root: tree } });
  assert.deepEqual(lookup(callId(OWNER, ["order.py::order"])), { key: "order.py::order", name: "order" });
  assert.deepEqual(lookup(callId(OWNER, ["?lib/x.py::lib"])), { key: null, name: "lib" });
  assert.equal(lookup(callId("elsewhere", ["x"])), undefined);
});

const node = (id: string, children: TreeNode[] = []): TreeNode => ({ id, kind: "function", name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
const structure = node("/", [node("f"), node("g"), node("h#2")]);

test("callExists checks the keys inside the id against the structure, without a call tree", () => {
  assert.equal(callExists(structure, callId("f", ["g"])), true);
  assert.equal(callExists(structure, callId("f", ["gone"])), false);
  assert.equal(callExists(structure, callId("gone", ["g"])), false, "the owner is gone");
  assert.equal(callExists(structure, callId("f", ["?lib.py::lib"])), true, "an unknown callee cannot be checked: it counts as there");
  assert.equal(callExists(structure, callId("f", ["g#2"])), true, "a twin step is checked by its key");
  assert.equal(callExists(structure, callId("f", ["h#2"])), true, "a key that itself ends in #2 is found as it is");
  assert.equal(callExists(structure, "f"), false, "not a call id");
});

// moved from webview/src/canvas/calls.test.ts: both sides read call trees
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

test("a site in the parent's file is named by its line, in another file by the file's name and the line", () => {
  assert.equal(siteText(siteAt(11, "pkg/child.py"), "pkg/child.py"), "line 12");
  assert.equal(siteText(siteAt(11, "pkg/base.py"), "pkg/child.py"), "base.py:12");
  assert.equal(siteText(siteAt(0, "base.py"), "pkg/child.py"), "base.py:1", "a file at the top of the folder has no directory to cut");
});

test("a site with no file is not listed, because there is nowhere to open it", () => {
  const b = call("b", { call_sites: [siteAt(3, "app.py"), siteAt(4, null), siteAt(5, "lib/x.py")] });
  assert.deepEqual(listedSites(b).map((s) => s.start.line), [3, 5]);
  assert.deepEqual(listedSites(call("c", { call_sites: [siteAt(1, null)] })), []);
});
