import assert from "node:assert/strict";
import { test } from "node:test";

import type { TreeNode } from "./messages";
import { type ContextState, type Intent, type Session, emptySession, keepEqual, reduce, restoreSession, toSaved } from "./session.ts";

const node = (id: string, kind: TreeNode["kind"], children: TreeNode[] = []): TreeNode => ({
  id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children,
});

// /  ├ pkg ├ a.py ├ C ├ m      /  ├ b.py ├ f
const tree = node("/", "folder", [node("pkg", "folder", [node("a.py", "file", [node("C", "class", [node("m", "function")])])]), node("b.py", "file", [node("f", "function")])]);

const run = (s: Session, ...intents: Intent[]) => intents.reduce((state, intent) => reduce(state, "main", intent, tree), s);
const main = (s: Session): ContextState => s.contexts.main;

test("focus pushes and selects; select never focuses", () => {
  const focused = run(emptySession(), { type: "focus", id: "pkg" });
  assert.deepEqual(main(focused).focus, ["pkg"]);
  assert.equal(main(focused).selected, "pkg");
  const selected = run(focused, { type: "select", id: "m" });
  assert.deepEqual(main(selected).focus, ["pkg"], "selecting inside a focus does not move the crumbs");
  assert.equal(main(selected).selected, "m");
});

test("focusing a node lower in the stack jumps back to it, and the node on top does nothing", () => {
  const s = run(emptySession(), { type: "focus", id: "pkg" }, { type: "focus", id: "a.py" }, { type: "focus", id: "C" });
  assert.deepEqual(main(s).focus, ["pkg", "a.py", "C"]);
  assert.deepEqual(main(run(s, { type: "focus", id: "pkg" })).focus, ["pkg"]);
  assert.equal(run(s, { type: "focus", id: "C" }), s, "already on top: the same object");
});

test("back keeps the selection when it is still under the new top, else selects the new top", () => {
  const s = run(emptySession(), { type: "focus", id: "pkg" }, { type: "focus", id: "a.py" }, { type: "select", id: "m" });
  const back = run(s, { type: "focusBack" });
  assert.deepEqual(main(back).focus, ["pkg"]);
  assert.equal(main(back).selected, "m", "m is under pkg");
  const elsewhere = run(emptySession(), { type: "focus", id: "pkg" }, { type: "focus", id: "b.py" }, { type: "select", id: "f" }, { type: "focusJump", depth: 1 });
  assert.equal(main(elsewhere).selected, "pkg", "f is not under pkg: the new top is selected");
  assert.deepEqual(main(run(elsewhere, { type: "focusJump", depth: 0 })).focus, []);
  assert.equal(run(emptySession(), { type: "focusBack" }).seq, 0, "nothing to go back to");
});

test("toggle, expand and collapseAll change only the open nodes", () => {
  const s = run(emptySession(), { type: "toggle", id: "pkg" }, { type: "expand", ids: ["a.py", "pkg"] });
  assert.deepEqual(main(s).expanded, ["pkg", "a.py"]);
  assert.deepEqual(main(run(s, { type: "toggle", id: "pkg" })).expanded, ["a.py"]);
  assert.deepEqual(main(run(s, { type: "collapseAll" })).expanded, []);
  const nothingOpen = emptySession();
  assert.equal(run(nothingOpen, { type: "collapseAll" }), nothingOpen, "nothing to close: the same object");
});

test("reveal steps the focus back, opens the ancestors and selects; it asks the canvas only when told to", () => {
  const s = run(emptySession(), { type: "focus", id: "pkg" }, { type: "focus", id: "a.py" });
  const shown = run(s, { type: "reveal", id: "f", canvas: true });
  assert.deepEqual(main(shown).focus, [], "nothing on the stack contains f");
  assert.deepEqual(main(shown).expanded, ["/", "b.py"], "every ancestor of f, not f itself");
  assert.equal(main(shown).selected, "f");
  assert.deepEqual(shown.reveal, { context: "main", id: "f", n: shown.seq });

  const inside = run(s, { type: "reveal", id: "m", canvas: false });
  assert.deepEqual(main(inside).focus, ["pkg", "a.py"], "m is inside the focus: kept");
  assert.deepEqual(main(inside).expanded, ["/", "pkg", "a.py", "C"]);
  assert.equal(inside.reveal, null, "the sidebar's reveal leaves nothing for the canvas");
  assert.deepEqual(main(run(s, { type: "reveal", id: "C", canvas: false })).focus, ["pkg", "a.py"]);
  assert.deepEqual(main(run(run(emptySession(), { type: "focus", id: "pkg" }, { type: "focus", id: "b.py" }), { type: "reveal", id: "C", canvas: false })).focus, ["pkg"], "b.py does not contain C, pkg does");
  assert.equal(run(s, { type: "reveal", id: "gone", canvas: true }), s, "a node that is not in the tree");
});

test("revealed clears the request, and only the one it names", () => {
  const shown = run(emptySession(), { type: "reveal", id: "m", canvas: true });
  const n = shown.reveal!.n;
  assert.equal(run(shown, { type: "revealed", n: n - 1 }), shown, "an older one: nothing");
  assert.equal(run(shown, { type: "revealed", n }).reveal, null);
  const again = run(shown, { type: "revealed", n }, { type: "reveal", id: "m", canvas: true });
  assert.notEqual(again.reveal!.n, n, "a new request never reuses a number the canvas already answered");
});

test("highlight changes nothing else, and select and focus clear it", () => {
  const s = run(emptySession(), { type: "select", id: "m" }, { type: "highlight", id: "f" });
  assert.equal(s.highlight, "f");
  assert.equal(main(s).selected, "m");
  assert.deepEqual(main(s).focus, []);
  assert.equal(run(s, { type: "select", id: "C" }).highlight, null);
  assert.equal(run(s, { type: "focus", id: "pkg" }).highlight, null);
  assert.equal(run(s, { type: "highlight", id: "f" }), s);
});

test("cursor changes nothing else, and is not part of what is saved", () => {
  const s = run(emptySession(), { type: "select", id: "m" });
  const moved = run(s, { type: "cursor", id: "f" });
  assert.equal(moved.cursor, "f");
  assert.equal(moved.contexts, s.contexts, "the contexts object is the one it was");
  assert.deepEqual(toSaved(moved), toSaved(s));
});

test("treeChanged prunes every context, and what the session points at", () => {
  const second: ContextState = { id: "x", title: "X", parent: "main", focus: ["pkg", "gone2"], selected: "gone3", expanded: ["pkg", "gone4"] };
  const s0 = run(emptySession(), { type: "focus", id: "b.py" }, { type: "focus", id: "f" }, { type: "expand", ids: ["b.py", "gone"] }, { type: "cursor", id: "f" }, { type: "highlight", id: "f" });
  const s: Session = { ...s0, contexts: { ...s0.contexts, x: second }, order: ["main", "x"] };
  const smaller = node("/", "folder", [node("pkg", "folder"), node("b.py", "file")]); // f is gone, and so is "gone"
  const next = reduce(s, "main", { type: "treeChanged" }, smaller);
  assert.deepEqual(next.contexts.main.focus, ["b.py"], "one step back, not looking at nothing");
  assert.deepEqual(next.contexts.main.expanded, ["b.py"]);
  assert.deepEqual([next.cursor, next.highlight], [null, null]);
  assert.deepEqual(next.contexts.x.focus, ["pkg"]);
  assert.equal(next.contexts.x.selected, null);
  assert.deepEqual(next.contexts.x.expanded, ["pkg"]);
  assert.equal(reduce(next, "main", { type: "treeChanged" }, smaller), next, "nothing left to drop: the same object");
});

test("a selected node that goes Missing is dropped; the rest of the session stays", () => {
  const s = run(emptySession(), { type: "focus", id: "pkg" }, { type: "select", id: "m" }, { type: "expand", ids: ["pkg"] });
  const withoutM = node("/", "folder", [node("pkg", "folder", [node("a.py", "file", [node("C", "class")])]), node("b.py", "file", [node("f", "function")])]);
  const next = reduce(s, "main", { type: "treeChanged" }, withoutM);
  assert.equal(main(next).selected, null);
  assert.deepEqual([main(next).focus, main(next).expanded], [["pkg"], ["pkg"]], "the focus and the open cards are untouched");
});

test("an intent that changes nothing, or names a context that does not exist, returns the same object; arrays that did not move are kept", () => {
  const s = run(emptySession(), { type: "focus", id: "pkg" }, { type: "expand", ids: ["pkg"] });
  assert.equal(run(s, { type: "select", id: "pkg" }), s);
  assert.equal(reduce(s, "nope", { type: "focus", id: "b.py" }, tree), s);
  const selected = run(s, { type: "select", id: "m" });
  assert.equal(main(selected).focus, main(s).focus, "only the selection moved: focus is the same array");
  assert.equal(main(selected).expanded, main(s).expanded);
  assert.equal(run(s, { type: "expand", ids: ["pkg"] }), s);
});

test("keepEqual reuses the arrays whose contents did not change", () => {
  const old = run(emptySession(), { type: "focus", id: "pkg" }, { type: "expand", ids: ["pkg", "a.py"] });
  const arrived: Session = JSON.parse(JSON.stringify({ ...old, cursor: "m", seq: old.seq + 1 }));
  assert.notEqual(arrived.contexts.main.focus, old.contexts.main.focus, "a message has brand-new arrays");
  const kept = keepEqual(old, arrived);
  assert.equal(kept.contexts.main.focus, old.contexts.main.focus);
  assert.equal(kept.contexts.main.expanded, old.contexts.main.expanded);
  assert.equal(kept.order, old.order);
  assert.equal(kept.cursor, "m");
  const asked = run(old, { type: "reveal", id: "m", canvas: true });
  assert.equal(keepEqual(asked, JSON.parse(JSON.stringify({ ...asked, cursor: "f", seq: asked.seq + 1 }))).reveal, asked.reveal, "the same request is the same object");
  const moved = keepEqual(old, { ...arrived, contexts: { main: { ...arrived.contexts.main, focus: ["pkg", "a.py"] } } });
  assert.deepEqual(moved.contexts.main.focus, ["pkg", "a.py"], "a real change comes through");
});

test("a saved session comes back; anything that does not parse is dropped; the old focus key is read once", () => {
  const s = run(emptySession(), { type: "focus", id: "pkg" }, { type: "expand", ids: ["a.py"] });
  const back = restoreSession(JSON.parse(JSON.stringify(toSaved(s))));
  assert.deepEqual(main(back).focus, ["pkg"]);
  assert.deepEqual(main(back).expanded, ["a.py"]);
  assert.deepEqual([back.cursor, back.highlight, back.reveal, back.seq], [null, null, null, 0]);
  assert.deepEqual(restoreSession({ contexts: 5 }), emptySession());
  assert.deepEqual(restoreSession(undefined), emptySession());
  assert.deepEqual(restoreSession({ contexts: { main: { id: "main" } }, order: ["main"], active: "main" }), emptySession(), "a context with a missing field");
  assert.deepEqual(main(restoreSession(undefined, ["a", "b"])).focus, ["a", "b"], "the old key");
  assert.deepEqual(main(restoreSession(undefined, "nonsense")).focus, []);
});
