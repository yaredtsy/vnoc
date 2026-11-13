import assert from "node:assert/strict";
import { test } from "node:test";

import { callExists, callId, callLookup, callNodesIn, callsLook, callsOf, isCallId, isStale, isStaleIn, listedSites, parseCallId, resolveCall, siteLabel, siteText, stepOf, wantedCalls } from "./calls.ts";
import type { CallTreeNode, CallTreeResult, Range, TreeNode } from "./messages";
import { type ContextState, type Session, emptySession, exploreId } from "./session.ts";

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

test("the lookup names the callee, key null for one the project does not know, undefined for a tree we do not hold, null for a call that is gone", () => {
  const lookup = callLookup({ [OWNER]: { root: tree } });
  assert.deepEqual(lookup(callId(OWNER, ["order.py::order"])), { key: "order.py::order", name: "order" });
  assert.deepEqual(lookup(callId(OWNER, ["?lib/x.py::lib"])), { key: null, name: "lib" });
  assert.equal(lookup(callId("elsewhere", ["x"])), undefined, "the owner's tree is not held: unknown");
  assert.equal(lookup(callId(OWNER, ["order.py::order", "nope"])), null, "the tree is held and the call is not in it: gone");
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

test("isStaleIn: stale against a newer revision of the same run of the core, never across a restart", () => {
  const t = (revision: number, epoch?: number): CallTreeResult => ({ revision, epoch, root: tree });
  assert.equal(isStaleIn(t(3, 1), { revision: 4, epoch: 1 }), true);
  assert.equal(isStaleIn(t(3, 1), { revision: 3, epoch: 1 }), false);
  assert.equal(isStaleIn(t(40, 1), { revision: 3, epoch: 2 }), false, "revision 40 of the old core and 3 of the new one say nothing about which is newer");
  assert.equal(isStaleIn(t(3, 2), { revision: 40, epoch: 1 }), false, "and the other way round");
  assert.equal(isStaleIn(t(3), { revision: 4 }), true, "no epochs: as before");
});

// ── callsOf (05 §4) ─────────────────────────────────────────────────────────────────────────────────────────────────

const names = (c: ReturnType<typeof callsOf>) => (c.state === "known" ? c.children.map((n) => n.name) : c.state);
const cutTree = call("checkout", { call_sites: [] }, [call("order", { marks: ["cut"] }), call("leaf", { marks: ["recursive"] }), call("lib", { key: null, marks: ["cut"] })]);
const orderTree = call("order", { call_sites: [] }, [call("reserve"), call("charge")]);

const callsOfTable: { name: string; store: Parameters<typeof callsOf>[0]; id: string; expect: string | string[] }[] = [
  { name: "a function with its tree: the root's children", store: { trees: { [OWNER]: { root: tree } } }, id: OWNER, expect: ["order", "lib"] },
  { name: "a function with no tree and nothing happening: unknown (a chevron, no +N)", store: { trees: {} }, id: OWNER, expect: "unknown" },
  { name: "a function a probe has answered for: probed, with its count", store: { trees: {}, probes: { [OWNER]: { count: 3 } } }, id: OWNER, expect: "probed" },
  { name: "a probe of 0: probed, with nothing to open", store: { trees: {}, probes: { [OWNER]: { count: 0 } } }, id: OWNER, expect: "probed" },
  { name: "its tree wins over its probe once it is here", store: { trees: { [OWNER]: { root: tree } }, probes: { [OWNER]: { count: 99 } } }, id: OWNER, expect: ["order", "lib"] },
  { name: "being fetched wins over the probe: loading", store: { trees: {}, states: { [OWNER]: { state: "loading" } }, probes: { [OWNER]: { count: 3 } } }, id: OWNER, expect: "loading" },
  { name: "a cut call node whose callee is probed: probed", store: { trees: { [OWNER]: { root: cutTree } }, probes: { "order.py::order": { count: 4 } } }, id: callId(OWNER, ["order.py::order"]), expect: "probed" },
  { name: "a function whose tree is being fetched: loading", store: { trees: {}, states: { [OWNER]: { state: "loading" } } }, id: OWNER, expect: "loading" },
  { name: "a function whose fetch failed: failed", store: { trees: {}, states: { [OWNER]: { state: "failed", message: "boom" } } }, id: OWNER, expect: "failed" },
  { name: "a call node: its own children in the owner's tree", store: { trees: { [OWNER]: { root: tree } } }, id: callId(OWNER, ["order.py::order"]), expect: ["reserve", "check"] },
  { name: "a call node that is recursive: nothing under it", store: { trees: { [OWNER]: { root: cutTree } } }, id: callId(OWNER, ["leaf.py::leaf"]), expect: [] },
  { name: "a call node cut by the walk, its callee's tree held: the callee's root children", store: { trees: { [OWNER]: { root: cutTree }, "order.py::order": { root: orderTree } } }, id: callId(OWNER, ["order.py::order"]), expect: ["reserve", "charge"] },
  { name: "a cut call node, its callee's tree not held: unknown", store: { trees: { [OWNER]: { root: cutTree } } }, id: callId(OWNER, ["order.py::order"]), expect: "unknown" },
  { name: "a cut call node, its callee's tree being fetched: loading", store: { trees: { [OWNER]: { root: cutTree } }, states: { "order.py::order": { state: "loading" } } }, id: callId(OWNER, ["order.py::order"]), expect: "loading" },
  { name: "a cut call node the project does not know (no key): nothing to open", store: { trees: { [OWNER]: { root: cutTree } } }, id: callId(OWNER, ["?lib.py::lib"]), expect: [] },
  { name: "a call that is gone from a held tree: nothing under it", store: { trees: { [OWNER]: { root: tree } } }, id: callId(OWNER, ["order.py::order", "gone"]), expect: [] },
  { name: "a call whose owner's tree is not held: what the owner's state says", store: { trees: {}, states: { [OWNER]: { state: "loading" } } }, id: callId(OWNER, ["order.py::order"]), expect: "loading" },
];
for (const row of callsOfTable) test(`callsOf: ${row.name}`, () => assert.deepEqual(names(callsOf(row.store, row.id)), row.expect));

test("callsOf: a failed fetch carries its message", () => {
  assert.deepEqual(callsOf({ trees: {}, states: { [OWNER]: { state: "failed", message: "boom" } } }, OWNER), { state: "failed", message: "boom" });
});

// ── wantedCalls (05 §2) ─────────────────────────────────────────────────────────────────────────────────────────────

const entry = (id: string, kind: TreeNode["kind"], children: TreeNode[] = []): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
// / ├ app.py ├ f ├ g ├ K (class, no __init__ of its own: it may inherit one) ├ h          ├ other.py
//        └ W (class with its own __init__ ├ W.__init__ ├ W.run)
const named = (n: TreeNode, name: string): TreeNode => ({ ...n, name });
const project = entry("/", "folder", [
  entry("app.py", "file", [entry("f", "function"), entry("g", "function"), entry("K", "class", [entry("K.run", "function")]), entry("h", "function"), entry("W", "class", [named(entry("W.__init__", "function"), "__init__"), entry("W.run", "function")])]),
  entry("other.py", "file"),
]);

const sessionWith = (main: Partial<ContextState>, ...more: ContextState[]): Session => {
  const s = emptySession();
  const contexts: Record<string, ContextState> = { main: { ...s.contexts.main, ...main }, ...Object.fromEntries(more.map((c) => [c.id, c])) };
  return { ...s, contexts, order: ["main", ...more.map((c) => c.id)] };
};
const explore = (call: string, over: Partial<ContextState> = {}): ContextState => ({ id: exploreId(call), title: "x", kind: "explore", parent: "main", call, focus: [], selected: null, expanded: [], ...over });
const Cfg = callId("f", ["g"]);
const Chg = callId("h", ["g"]);
const trees = {};
const keys = (w: ReturnType<typeof wantedCalls>) => w.map((x) => `${x.key}:${x.reason}`);

const wantedTable: { name: string; session: Session; trees?: Parameters<typeof wantedCalls>[2]; expect: string[] }[] = [
  { name: "nothing open, nothing selected: nothing", session: sessionWith({}), expect: [] },
  { name: "an open function", session: sessionWith({ expanded: ["f"] }), expect: ["f:open"] },
  { name: "an open class without an __init__ of its own (it may inherit one: its tree is what that calls)", session: sessionWith({ expanded: ["K"] }), expect: ["K:open"] },
  { name: "a class with its own __init__ is not wanted: its __init__ is, when open", session: sessionWith({ expanded: ["W", "W.__init__"] }), expect: ["W.__init__:open"] },
  { name: "a class with its own __init__ is not wanted as the last focus step or the selection either", session: sessionWith({ focus: ["app.py", "W"], selected: "W" }), expect: [] },
  { name: "an open __init__ is an ordinary function", session: sessionWith({ expanded: ["W.__init__"] }), expect: ["W.__init__:open"] },
  { name: "a folder or a file that is open owns no calls", session: sessionWith({ expanded: ["app.py", "/"] }), expect: [] },
  { name: "a key that is not in the structure is not wanted", session: sessionWith({ expanded: ["ghost"] }), expect: [] },
  { name: "the last focus step, when it is a function", session: sessionWith({ focus: ["app.py", "f"] }), expect: ["f:open"] },
  { name: "a focus that ends at a file wants nothing", session: sessionWith({ focus: ["app.py"] }), expect: [] },
  { name: "an open call card wants its owner's tree", session: sessionWith({ expanded: [Cfg] }), expect: ["f:open"] },
  { name: "open call cards in every context: each owner is wanted", session: sessionWith({ expanded: [Cfg], selected: Cfg }, explore(Cfg, { expanded: [Chg] })), expect: ["f:open", "h:open"] },
  { name: "a focus step that is a call: the EXPLORE context focused on the callee wants the callee's tree", session: sessionWith({ expanded: [Cfg], selected: Cfg }, explore(Cfg, { focus: ["app.py", "g"] })), expect: ["f:open", "g:open"] },
  { name: "the selected function of the active context is prefetched, last", session: sessionWith({ expanded: ["f"], selected: "h" }), expect: ["f:open", "h:prefetch"] },
  { name: "a selected function that is also open is open, not prefetched, and not twice", session: sessionWith({ expanded: ["f"], selected: "f" }), expect: ["f:open"] },
  { name: "a selected call or file is not prefetched", session: sessionWith({ selected: Cfg }), expect: [] },
  { name: "only the active context's selection is prefetched", session: { ...sessionWith({ selected: "h" }, explore(Cfg, { selected: "g" })), active: "main" }, expect: ["h:prefetch"] },
  { name: "nothing is wanted twice: the same function open in two contexts, and as a focus step", session: sessionWith({ expanded: ["f"], focus: ["app.py", "f"] }, explore(Cfg, { expanded: ["f", Cfg] })), expect: ["f:open"] },
  { name: "a closed card stops being wanted", session: sessionWith({ expanded: [] }), expect: [] },
  {
    name: "an open call node the walk cut, with no children: its callee's own tree is wanted, after what is open and before the prefetch",
    session: sessionWith({ expanded: [Cfg, "h"], selected: "K" }),
    trees: { f: { root: call("f", { call_sites: [] }, [call("g", { key: "g", marks: ["cut"] })]) } },
    expect: ["f:open", "h:open", "g:cut", "K:prefetch"],
  },
  {
    name: "a cut call node that has children, or no key, or is not open, wants nothing more",
    session: sessionWith({ expanded: [Cfg] }),
    trees: { f: { root: call("f", { call_sites: [] }, [call("g", { key: "g", marks: ["cut"] }, [call("x")])]) } },
    expect: ["f:open"],
  },
];
for (const row of wantedTable) test(`wantedCalls: ${row.name}`, () => assert.deepEqual(keys(wantedCalls(row.session, project, row.trees ?? trees)), row.expect));

test("callsOf: a probe's count is what a probed function carries", () => {
  assert.deepEqual(callsOf({ trees: {}, probes: { f: { count: 3 } } }, "f"), { state: "probed", count: 3 });
});

// ── what a card shows (04 §1, 05 §3.1) ─────────────────────────────────────────────────────────────────────────────

const lookTable: { name: string; calls: Parameters<typeof callsLook>[0]; open: boolean; expect: ReturnType<typeof callsLook> }[] = [
  { name: "pending (a probe is out): an empty slot, nothing said", calls: { state: "unknown" }, open: false, expect: { chevron: false, plus: null, callsNothing: false } },
  { name: "probed with calls, closed: a chevron and +N", calls: { state: "probed", count: 3 }, open: false, expect: { chevron: true, plus: 3, callsNothing: false } },
  { name: "probed with calls, open: a chevron, no +N", calls: { state: "probed", count: 3 }, open: true, expect: { chevron: true, plus: null, callsNothing: false } },
  { name: "probed with 0: no chevron, calls nothing", calls: { state: "probed", count: 0 }, open: false, expect: { chevron: false, plus: null, callsNothing: true } },
  { name: "known with children, closed: a chevron and +N", calls: { state: "known", children: [call("a"), call("b")] }, open: false, expect: { chevron: true, plus: 2, callsNothing: false } },
  { name: "known and empty: calls nothing, and the chevron goes", calls: { state: "known", children: [] }, open: true, expect: { chevron: false, plus: null, callsNothing: true } },
  { name: "loading: the chevron is there, nothing counted", calls: { state: "loading" }, open: true, expect: { chevron: true, plus: null, callsNothing: false } },
  { name: "failed: the chevron stays, so there is a way to try again", calls: { state: "failed", message: "boom" }, open: true, expect: { chevron: true, plus: null, callsNothing: false } },
];
for (const row of lookTable) test(`callsLook: ${row.name}`, () => assert.deepEqual(callsLook(row.calls, row.open), row.expect));
