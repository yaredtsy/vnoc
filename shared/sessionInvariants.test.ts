import assert from "node:assert/strict";
import { test } from "node:test";

import { callId } from "./calls.ts";
import type { TreeNode } from "./messages";
import { type ContextState, type Session, emptySession } from "./session.ts";
import { sessionProblems } from "./sessionInvariants.ts";

const node = (id: string, kind: TreeNode["kind"], children: TreeNode[] = []): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
const tree = node("/", "folder", [node("a.py", "file", [node("f", "function"), node("g", "function")])]);
const CALL = callId("f", ["g"]);

const explore = (over: Partial<ContextState> = {}): ContextState => ({ id: "x", title: "g", kind: "explore", parent: "main", call: CALL, focus: ["a.py", "g"], selected: "g", expanded: [], ...over });
const withExplore = (c: ContextState, mainSelected: string | null = CALL, order = ["main", c.id]): Session => {
  const s = emptySession();
  return { ...s, contexts: { main: { ...s.contexts.main, selected: mainSelected }, [c.id]: c }, order };
};

test("a fresh session and a sound one with an EXPLORE context have no problems", () => {
  assert.deepEqual(sessionProblems(emptySession(), tree), []);
  assert.deepEqual(sessionProblems(withExplore(explore()), tree), []);
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
  ["order and contexts that differ", { ...emptySession(), order: ["main", "ghost"] }, /do not hold the same contexts/],
];
for (const [name, session, expected] of broken) {
  test(`the checker notices ${name}`, () => assert.match(sessionProblems(session, tree).join("\n"), expected));
}
