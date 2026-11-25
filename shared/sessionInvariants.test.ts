import assert from "node:assert/strict";
import { test } from "node:test";

import { callId } from "./calls.ts";
import type { TreeNode } from "./messages";
import { type ContextState, type Session, emptySession, tourContextId } from "./session.ts";
import { sessionProblems } from "./sessionInvariants.ts";

const node = (id: string, kind: TreeNode["kind"], children: TreeNode[] = []): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
const tree = node("/", "folder", [node("a.py", "file", [node("f", "function"), node("g", "function")])]);
const CALL = callId("f", ["g"]);

const explore = (over: Partial<ContextState> = {}): ContextState => ({ id: "x", title: "g", kind: "explore", parent: "main", call: CALL, tour: null, preview: false, focus: ["a.py", "g"], selected: "g", open: {}, recent: [], ...over });
const withExplore = (c: ContextState, mainSelected: string | null = CALL, order = ["main", c.id]): Session => {
  const s = emptySession();
  return { ...s, contexts: { main: { ...s.contexts.main, selected: mainSelected }, [c.id]: c }, order };
};

test("a fresh session and a sound one with an EXPLORE context have no problems", () => {
  assert.deepEqual(sessionProblems(emptySession(), tree), []);
  assert.deepEqual(sessionProblems(withExplore(explore()), tree), []);
});

const tourCtx = (over: Partial<ContextState> = {}): ContextState => ({ id: tourContextId("t-1"), title: "t", kind: "tour", parent: "main", call: null, tour: "t-1", preview: false, focus: [], selected: null, open: {}, recent: [], ...over });
const withTours = (...tours: ContextState[]): Session => {
  const s = emptySession();
  return { ...s, contexts: { main: s.contexts.main, ...Object.fromEntries(tours.map((c) => [c.id, c])) }, order: ["main", ...tours.map((c) => c.id)] };
};

test("a session with a TOUR context is sound", () => {
  assert.deepEqual(sessionProblems(withTours(tourCtx()), tree), []);
  assert.deepEqual(sessionProblems(withTours(tourCtx({ selected: "g" })), tree), []);
});

// the checker is only worth running if it notices each way a session can be wrong
const broken: [string, Session, RegExp][] = [
  ["active is not open", { ...emptySession(), active: "gone" }, /active "gone" is not an open context/],
  ["an EXPLORE parent that is not open", withExplore(explore({ parent: "nope" })), /its parent nope is not open/],
  ["a parent that comes after its child", withExplore(explore(), CALL, ["x", "main"]), /does not come before it|MAIN is not first/],
  ["the parent has not selected the call", withExplore(explore(), null), /has not selected its call/],
  ["two EXPLORE contexts for one call", { ...withExplore(explore()), contexts: { ...withExplore(explore()).contexts, y: explore({ id: "y" }) }, order: ["main", "x", "y"] }, /a second EXPLORE context for the same call/],
  ["a focus that is not a path", withExplore(explore({ focus: ["g"] })), /is not a path in the tree/],
  ["a selection that is gone", { ...emptySession(), contexts: { main: { ...emptySession().contexts.main, selected: "ghost" } } }, /selected "ghost" does not exist/],
  ["a call that is not in the tree", withExplore(explore({ call: callId("f", ["ghost"]) }), callId("f", ["ghost"])), /its call does not exist/],
  ["a MAIN that is a preview", { ...emptySession(), contexts: { main: { ...emptySession().contexts.main, preview: true } } }, /MAIN is never a preview/],
  ["an active context that is a preview", { ...withExplore(explore({ preview: true })), active: "x" }, /the active context x is a preview/],
  ["a TOUR context that names no tour", withTours(tourCtx({ tour: null })), /names its tour/],
  ["a TOUR context whose id is not its tour's", withTours(tourCtx({ tour: "t-2" })), /names its tour/],
  ["a TOUR context with a call", withTours(tourCtx({ call: CALL })), /belongs to MAIN and has no call/],
  ["a TOUR context that is not MAIN's child", withTours(tourCtx({ parent: "main2" })), /belongs to MAIN/],
  ["two TOUR contexts", withTours(tourCtx(), tourCtx({ id: tourContextId("t-2"), tour: "t-2" })), /2 TOUR contexts are open/],
  ["an EXPLORE context with a tour", withExplore(explore({ tour: "t-1" })), /an EXPLORE context has no tour/],
  ["a tour list view that is neither", { ...emptySession(), tourList: { view: "all\0" as never } }, /tourList\.view/],
  ["order and contexts that differ", { ...emptySession(), order: ["main", "ghost"] }, /do not hold the same contexts/],
];
for (const [name, session, expected] of broken) {
  test(`the checker notices ${name}`, () => assert.match(sessionProblems(session, tree).join("\n"), expected));
}
