import assert from "node:assert/strict";
import { test } from "node:test";

import { type CallId, callId, callLookup } from "./calls.ts";
import type { CallTreeNode, TreeNode } from "./messages";
import { type ContextState, type Intent, MAIN, type Session, emptySession, exploreId, keepEqual, openOf, reduce, restoreSession, toSaved } from "./session.ts";
import { sessionProblems } from "./sessionInvariants.ts";

const node = (id: string, kind: TreeNode["kind"], children: TreeNode[] = []): TreeNode => ({
  id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children,
});

// /  ├ pkg ├ a.py ├ C ├ m          /  ├ b.py ├ f
//                                          └ g
const tree = node("/", "folder", [node("pkg", "folder", [node("a.py", "file", [node("C", "class", [node("m", "function")])])]), node("b.py", "file", [node("f", "function"), node("g", "function")])]);

const call = (key: string | null, name: string, children: CallTreeNode[] = [], marks: CallTreeNode["marks"] = []): CallTreeNode => ({
  key, name, qualname: name, path: key === null ? null : `${key}.py`, kind: "function", call_count: 1, call_sites: [], marks, children,
});
// the call tree of f:   f ├ m ├ g ├ f (recursive)        f calls m, g and a library; m calls g; g calls f again
//                         ├ g
//                         └ lib (the project does not know it)
const trees = { f: { root: call("f", "f", [call("m", "m", [call("g", "g", [call("f", "f", [], ["recursive"])])]), call("g", "g"), call(null, "lib")]) } };
const lookup = callLookup(trees);

const Cm = callId("f", ["m"]);
const Cg = callId("f", ["g"]);
const Cmg = callId("f", ["m", "g"]);
const Cmgf = callId("f", ["m", "g", "f"]);
const Clib = callId("f", ["?::lib"]);
const Cunheld = callId("m", ["g"]); // m is in the project, but we hold no call tree for it

/** Applies the intents in a context, and checks the session is sound after EVERY step: a rule broken in one intent shows in the test of any other. */
const runIn = (context: string, s: Session, ...intents: Intent[]): Session =>
  intents.reduce((state, intent) => {
    const next = reduce(state, context, intent, tree, lookup);
    assert.deepEqual(sessionProblems(next, tree), [], `after ${JSON.stringify(intent)}`);
    return next;
  }, s);
const run = (s: Session, ...intents: Intent[]) => runIn(MAIN, s, ...intents);
const ctx = (s: Session, id: string = MAIN): ContextState => s.contexts[id];

// ── 03 §3: the focus is a path ────────────────────────────────────────────────────────────────────────────────────────

// each row: the intents before, the intent, then what the focus and the selection are
const focusTable: { name: string; before: Intent[]; intent: Intent; focus: string[]; selected: string | null }[] = [
  { name: "focus on a deep node gives its lineage, and selects it", before: [], intent: { type: "focus", id: "m" }, focus: ["pkg", "a.py", "C", "m"], selected: "m" },
  { name: "focus on a top-level node", before: [], intent: { type: "focus", id: "pkg" }, focus: ["pkg"], selected: "pkg" },
  { name: "focus on another branch replaces the path, it does not add to it", before: [{ type: "focus", id: "m" }], intent: { type: "focus", id: "f" }, focus: ["b.py", "f"], selected: "f" },
  { name: "focus on an ancestor of the focus shortens the path", before: [{ type: "focus", id: "m" }], intent: { type: "focus", id: "a.py" }, focus: ["pkg", "a.py"], selected: "a.py" },
  { name: "Back drops the last step; the selection stays when it is still under the new last step", before: [{ type: "focus", id: "m" }, { type: "select", id: "C" }], intent: { type: "focusBack" }, focus: ["pkg", "a.py", "C"], selected: "C" },
  { name: "Back keeps a selection that is still under the new last step", before: [{ type: "focus", id: "f" }], intent: { type: "focusBack" }, focus: ["b.py"], selected: "f" },
  { name: "Back selects the new last step when the selection is not under it", before: [{ type: "focus", id: "f" }, { type: "select", id: "m" }], intent: { type: "focusBack" }, focus: ["b.py"], selected: "b.py" },
  { name: "Back walks up to the project", before: [{ type: "focus", id: "pkg" }], intent: { type: "focusBack" }, focus: [], selected: "pkg" },
  { name: "a crumb jumps to the first n steps; a selection under it stays", before: [{ type: "focus", id: "m" }], intent: { type: "focusJump", depth: 2 }, focus: ["pkg", "a.py"], selected: "m" },
  { name: "a crumb selects its own card when the selection is not under it", before: [{ type: "focus", id: "m" }, { type: "select", id: "f" }], intent: { type: "focusJump", depth: 2 }, focus: ["pkg", "a.py"], selected: "a.py" },
  { name: "the project crumb clears the focus and keeps the selection", before: [{ type: "focus", id: "m" }], intent: { type: "focusJump", depth: 0 }, focus: [], selected: "m" },
  { name: "reveal outside the focus drops steps from the end until the last contains the node", before: [{ type: "focus", id: "m" }], intent: { type: "reveal", id: "C", canvas: false }, focus: ["pkg", "a.py", "C"], selected: "C" },
  { name: "reveal in another branch drops the whole path", before: [{ type: "focus", id: "m" }], intent: { type: "reveal", id: "f", canvas: true }, focus: [], selected: "f" },
  { name: "reveal inside the focus never adds a step", before: [{ type: "focus", id: "a.py" }], intent: { type: "reveal", id: "m", canvas: false }, focus: ["pkg", "a.py"], selected: "m" },
];
for (const row of focusTable) {
  test(`03 §3: ${row.name}`, () => {
    const s = run(emptySession(), ...row.before, row.intent);
    assert.deepEqual([...ctx(s).focus], row.focus);
    assert.equal(ctx(s).selected, row.selected);
  });
}

test("03 §3: focusing the last step, a node that is not in the tree, or Back with nothing to go back to, does nothing", () => {
  const s = run(emptySession(), { type: "focus", id: "m" });
  assert.equal(run(s, { type: "focus", id: "m" }), s);
  assert.equal(run(s, { type: "focus", id: "gone" }), s);
  const fresh = emptySession();
  assert.equal(run(fresh, { type: "focusBack" }), fresh);
  assert.equal(run(s, { type: "focusJump", depth: 9 }), s, "a crumb past the end");
});

test("select never moves the focus", () => {
  const s = run(emptySession(), { type: "focus", id: "pkg" }, { type: "select", id: "m" });
  assert.deepEqual([...ctx(s).focus], ["pkg"]);
  assert.equal(ctx(s).selected, "m");
});

// ── 03 §4: contexts ───────────────────────────────────────────────────────────────────────────────────────────────────

test("03 §4: selecting a call opens an EXPLORE child for its callee: not active, its focus the callee's lineage, the callee selected", () => {
  const s = run(emptySession(), { type: "select", id: Cm });
  assert.deepEqual(s.order, [MAIN, exploreId(Cm)]);
  assert.equal(s.active, MAIN, "the canvas stays where it was");
  assert.equal(ctx(s).selected, Cm, "the call is the parent's selection");
  const child = ctx(s, exploreId(Cm));
  assert.deepEqual([child.kind, child.parent, child.call, child.title], ["explore", MAIN, Cm, "m"]);
  assert.deepEqual([...child.focus], ["pkg", "a.py", "C", "m"]);
  assert.equal(child.selected, "m");
});

test("03 §4: selecting another call closes the first child and opens a second", () => {
  const s = run(emptySession(), { type: "select", id: Cm }, { type: "select", id: Cg });
  assert.deepEqual(s.order, [MAIN, exploreId(Cg)]);
  assert.deepEqual([...ctx(s, exploreId(Cg)).focus], ["b.py", "g"]);
});

test("03 §4: selecting anything that is not that call closes the children (a structure node, nothing, or the same call's neighbour)", () => {
  for (const intent of [{ type: "select", id: "f" }, { type: "select", id: null }, { type: "focus", id: "pkg" }] as Intent[]) {
    const s = run(emptySession(), { type: "select", id: Cm }, intent);
    assert.deepEqual(s.order, [MAIN], JSON.stringify(intent));
  }
});

test("03 §4: Explore (double-click, ◇, Enter) does the same and activates the child; Focus on a call is Explore", () => {
  const explored = run(emptySession(), { type: "explore", call: Cm, activate: true });
  assert.equal(explored.active, exploreId(Cm));
  assert.equal(ctx(explored).selected, Cm);
  const focused = run(emptySession(), { type: "focus", id: Cm });
  assert.equal(focused.active, exploreId(Cm), "focus on a call follows it");
  const quiet = run(emptySession(), { type: "explore", call: Cm, activate: false });
  assert.equal(quiet.active, MAIN, "Explore without activate opens it and stays");
  assert.deepEqual(quiet.order, [MAIN, exploreId(Cm)]);
});

test("03 §4: activate (a click on a panel or a chip) switches the canvas; an unknown or the current context does nothing", () => {
  const s = run(emptySession(), { type: "select", id: Cm });
  assert.equal(run(s, { type: "activate", id: exploreId(Cm) }).active, exploreId(Cm));
  assert.equal(run(s, { type: "activate", id: "nope" }), s);
  assert.equal(run(s, { type: "activate", id: MAIN }), s);
});

test("03 §4: a call whose callee the project does not know is selected and opens nothing; so does a call whose tree we do not hold", () => {
  for (const unknown of [Clib, Cunheld]) {
    const s = run(emptySession(), { type: "select", id: unknown });
    assert.deepEqual(s.order, [MAIN], unknown);
    assert.equal(ctx(s).selected, unknown, "but it is selected");
  }
  const fresh = emptySession();
  assert.equal(run(fresh, { type: "explore", call: Clib, activate: true }), fresh, "Explore on an unknown callee: nothing, not even a selection");
  assert.equal(run(fresh, { type: "explore", call: "not a call id", activate: true }), fresh);
});

test("selecting the same call twice opens one EXPLORE, not two", () => {
  const once = run(emptySession(), { type: "select", id: Cm });
  const twice = run(once, { type: "select", id: Cm });
  assert.equal(twice, once, "nothing changed: the same object");
  assert.equal(twice.order.filter((id) => id !== MAIN).length, 1);
  const after = run(once, { type: "explore", call: Cm, activate: true }, { type: "explore", call: Cm, activate: true });
  assert.equal(after.order.length, 2);
});

test("selecting a call inside an EXPLORE context opens a grandchild under it, depth first", () => {
  const e1 = exploreId(Cm);
  const e2 = exploreId(Cmg);
  const s = run(emptySession(), { type: "select", id: Cm });
  const deeper = runIn(e1, s, { type: "select", id: Cmg });
  assert.deepEqual(deeper.order, [MAIN, e1, e2]);
  assert.equal(ctx(deeper, e2).parent, e1);
  assert.deepEqual([...ctx(deeper, e2).focus], ["b.py", "g"]);
  // a second child of MAIN would come after the whole subtree of the first, not between it and its child
  assert.equal(deeper.contexts[e1].selected, Cmg);
});

test("closing an EXPLORE of an EXPLORE (depth 2) closes its own children; the parent stays", () => {
  const [e1, e2, e3] = [exploreId(Cm), exploreId(Cmg), exploreId(Cmgf)];
  let s = run(emptySession(), { type: "select", id: Cm });
  s = runIn(e1, s, { type: "select", id: Cmg });
  s = runIn(e2, s, { type: "select", id: Cmgf });
  assert.deepEqual(s.order, [MAIN, e1, e2, e3]);
  const closed = run(s, { type: "closeContext", id: e2 });
  assert.deepEqual(closed.order, [MAIN, e1], "e2 and its own child e3 are gone, e1 stays");
  assert.deepEqual(ctx(closed, e1).selected, Cmg, "the parent's selection is not changed by closing a panel");
});

test("closing the middle one hands active to the nearest open ancestor", () => {
  const [e1, e2, e3] = [exploreId(Cm), exploreId(Cmg), exploreId(Cmgf)];
  let s = run(emptySession(), { type: "select", id: Cm });
  s = runIn(e1, s, { type: "select", id: Cmg });
  s = runIn(e2, s, { type: "select", id: Cmgf });
  s = run(s, { type: "activate", id: e3 });
  assert.equal(s.active, e3);
  assert.equal(run(s, { type: "closeContext", id: e2 }).active, e1, "e3 went with e2: the nearest ancestor that stays is e1");
  assert.equal(run(s, { type: "closeContext", id: e1 }).active, MAIN, "all three go: MAIN");
  const elsewhere = run(s, { type: "activate", id: e1 }, { type: "closeContext", id: e3 });
  assert.equal(elsewhere.active, e1, "closing one that is not active leaves the active one");
});

// ── preview (09 §4): an EXPLORE opened by selecting its call is a preview until it is activated once ───────────────────────────────

const isPreview = (s: Session, id: string) => ctx(s, id).preview;

test("09 §4: selecting a call opens its EXPLORE as a preview; MAIN never is", () => {
  const s = run(emptySession(), { type: "select", id: Cm });
  assert.equal(isPreview(s, exploreId(Cm)), true);
  assert.equal(isPreview(s, MAIN), false);
  assert.equal(isPreview(run(emptySession(), { type: "explore", call: Cm, activate: false }), exploreId(Cm)), true, "Explore without activating is a preview too");
});

// every way a context becomes the active one ends its preview (a table: how → the context it activates)
const activations: { name: string; steps: Intent[]; context: string }[] = [
  { name: "a click on its panel or chip (activate)", steps: [{ type: "select", id: Cm }, { type: "activate", id: exploreId(Cm) }], context: exploreId(Cm) },
  { name: "Explore with activate (a double-click)", steps: [{ type: "select", id: Cm }, { type: "explore", call: Cm, activate: true }], context: exploreId(Cm) },
  { name: "Focus on the call (it is Explore)", steps: [{ type: "select", id: Cm }, { type: "focus", id: Cm }], context: exploreId(Cm) },
  { name: "Explore with activate on a call that was never selected: opened active, never a preview", steps: [{ type: "explore", call: Cm, activate: true }], context: exploreId(Cm) },
];
for (const a of activations) {
  test(`09 §4: ${a.name} ends the preview`, () => {
    const s = run(emptySession(), ...a.steps);
    assert.equal(s.active, a.context);
    assert.equal(isPreview(s, a.context), false);
  });
}

test("09 §4: activating one context leaves another one's preview alone; it ends only when that one is activated", () => {
  const [e1, e2] = [exploreId(Cm), exploreId(Cmg)];
  let s = run(emptySession(), { type: "select", id: Cm });
  s = runIn(e1, s, { type: "select", id: Cmg }); // a grandchild, a preview too
  assert.deepEqual([isPreview(s, e1), isPreview(s, e2)], [true, true]);
  s = run(s, { type: "activate", id: e1 });
  assert.deepEqual([isPreview(s, e1), isPreview(s, e2)], [false, true]);
  s = run(s, { type: "activate", id: e2 }, { type: "activate", id: MAIN });
  assert.deepEqual([isPreview(s, e1), isPreview(s, e2)], [false, false], "going back to MAIN does not make it a preview again");
});

test("09 §4: when the active context closes, the ancestor that takes over is active, so it is no longer a preview", () => {
  const [e1, e2] = [exploreId(Cm), exploreId(Cmg)];
  let s = run(emptySession(), { type: "select", id: Cm });
  s = runIn(e1, s, { type: "select", id: Cmg });
  s = run(s, { type: "activate", id: e2 });
  assert.equal(isPreview(s, e1), true, "e1 was never activated");
  s = run(s, { type: "closeContext", id: e2 });
  assert.equal(s.active, e1);
  assert.equal(isPreview(s, e1), false);
});

test("09 §4: activating what is already active, or a context that is not a preview, changes nothing", () => {
  const s = run(emptySession(), { type: "select", id: Cm }, { type: "activate", id: exploreId(Cm) });
  assert.equal(run(s, { type: "activate", id: exploreId(Cm) }), s);
  assert.equal(run(s, { type: "activate", id: MAIN }).contexts[exploreId(Cm)].preview, false);
});

test("09 §4: the preview is saved, so a reloaded page keeps it; an active context never comes back as one", () => {
  const s = run(emptySession(), { type: "select", id: Cm });
  const back = roundTrip(s);
  assert.equal(isPreview(back, exploreId(Cm)), true);
  assert.deepEqual(sessionProblems(back, tree), []);
  const lying = JSON.parse(JSON.stringify(toSaved(s)));
  lying.active = exploreId(Cm); // a file that says the active context is a preview
  assert.equal(isPreview(restoreSession(lying), exploreId(Cm)), false);
  const old = JSON.parse(JSON.stringify(toSaved(s)));
  delete old.contexts[exploreId(Cm)].preview; // a session saved before the flag
  assert.equal(isPreview(restoreSession(old), exploreId(Cm)), false);
  const mainLie = JSON.parse(JSON.stringify(toSaved(s)));
  mainLie.contexts[MAIN].preview = true;
  assert.equal(isPreview(restoreSession(mainLie), MAIN), false, "MAIN is never a preview, whatever the file says");
});

test("closeContext on MAIN or on a context that is not there does nothing", () => {
  const s = run(emptySession(), { type: "select", id: Cm });
  assert.equal(run(s, { type: "closeContext", id: MAIN }), s);
  assert.equal(run(s, { type: "closeContext", id: "nope" }), s);
});

test("a recursive call (f → m → g → f): the EXPLORE for the inner f is its own context, and the ids stay distinct", () => {
  const e1 = exploreId(Cm);
  const e2 = exploreId(Cmg);
  let s = run(emptySession(), { type: "select", id: Cm });
  s = runIn(e1, s, { type: "select", id: Cmg });
  s = runIn(e2, s, { type: "select", id: Cmgf });
  assert.equal(new Set(s.order).size, 4);
  assert.deepEqual([...ctx(s, exploreId(Cmgf)).focus], ["b.py", "f"], "it shows f at home");
});

test("the same call opened from two places is one EXPLORE context (one per call)", () => {
  const s = run(emptySession(), { type: "select", id: Cm });
  const again = runIn(exploreId(Cm), s, { type: "select", id: Cm }); // the child's own selection is its callee, so this is a different place
  assert.equal(again.order.filter((id) => ctx(again, id).call === Cm).length, 1);
});

// ── the code changed ──────────────────────────────────────────────────────────────────────────────────────────────────

const without = (...gone: string[]) => {
  const drop = (n: TreeNode): TreeNode => ({ ...n, children: n.children.filter((c) => !gone.includes(c.id)).map(drop) });
  return drop(tree);
};
const changed = (s: Session, t: TreeNode) => {
  const next = reduce(s, MAIN, { type: "treeChanged" }, t, callLookup(trees));
  assert.deepEqual(sessionProblems(next, t), [], "sound against the NEW tree");
  return next;
};

const treeTable: { name: string; focus: string[]; selected: string | null; tree: TreeNode; expectFocus: string[]; expectSelected: string | null }[] = [
  { name: "the last step was deleted: the focus is the longest path that still exists", focus: ["pkg", "a.py", "C", "m"], selected: "m", tree: without("m"), expectFocus: ["pkg", "a.py", "C"], expectSelected: null },
  { name: "a step in the middle was deleted (a file renamed: it and what is under it have new ids)", focus: ["pkg", "a.py", "C", "m"], selected: "C", tree: without("a.py"), expectFocus: ["pkg"], expectSelected: null },
  { name: "everything was deleted: no focus", focus: ["pkg", "a.py"], selected: "a.py", tree: without("pkg"), expectFocus: [], expectSelected: null },
  { name: "nothing the session points at changed: the same focus and selection", focus: ["pkg", "a.py"], selected: "m", tree: without("g"), expectFocus: ["pkg", "a.py"], expectSelected: "m" },
  { name: "the selection went but the focus holds: only the selection clears", focus: ["b.py"], selected: "g", tree: without("g"), expectFocus: ["b.py"], expectSelected: null },
];
for (const row of treeTable) {
  test(`treeChanged: ${row.name}`, () => {
    const base = emptySession();
    const s: Session = { ...base, contexts: { main: { ...base.contexts.main, focus: row.focus, selected: row.selected } } };
    const next = changed(s, row.tree);
    assert.deepEqual([...ctx(next).focus], row.expectFocus);
    assert.equal(ctx(next).selected, row.expectSelected);
  });
}

test("treeChanged with nothing to drop returns the same object", () => {
  const s = run(emptySession(), { type: "focus", id: "pkg" }, { type: "toggle", id: "pkg" });
  assert.equal(changed(s, tree), s);
});

test("a saved 0.1 history [a, x] becomes the lineage of x on the first treeChanged", () => {
  const old = JSON.parse(JSON.stringify({ contexts: { main: { id: "main", title: "Main", parent: null, focus: ["b.py", "m"], selected: null, expanded: [] } }, order: ["main"], active: "main" }));
  const restored = restoreSession(old);
  assert.deepEqual([...ctx(restored).focus], ["b.py", "m"], "restored as it was saved: a history, not yet a path");
  assert.deepEqual([ctx(restored).kind, ctx(restored).call], ["main", null], "a context saved before EXPLORE existed is MAIN");
  assert.deepEqual([...ctx(changed(restored, tree)).focus], ["pkg", "a.py", "C", "m"]);
});

test("a 0.1 history whose last entry is gone becomes the lineage of the last one that exists; a path-shaped one is left alone", () => {
  const fromHistory = (focus: string[], t: TreeNode) => {
    const base = emptySession();
    return [...ctx(changed({ ...base, contexts: { main: { ...base.contexts.main, focus } } }, t)).focus];
  };
  assert.deepEqual(fromHistory(["a.py", "f"], without("f")), ["pkg", "a.py"], "f is gone: a.py's lineage");
  assert.deepEqual(fromHistory(["gone1", "gone2"], tree), [], "nothing exists");
  assert.deepEqual(fromHistory(["pkg", "a.py"], tree), ["pkg", "a.py"], "already a path");
  assert.deepEqual(fromHistory(["C", "pkg"], tree), ["pkg"], "the last entry that exists, pkg, is top-level");
});

test("treeChanged closes an EXPLORE context whose call is gone, takes its children with it, and clears the selection that pointed at it", () => {
  const e1 = exploreId(Cm);
  let s = run(emptySession(), { type: "select", id: Cm });
  s = runIn(e1, s, { type: "select", id: Cmg });
  s = run(s, { type: "activate", id: exploreId(Cmg) });
  const next = changed(s, without("m")); // m is gone: the call to it, and what was explored from it
  assert.deepEqual(next.order, [MAIN]);
  assert.equal(next.active, MAIN);
  assert.equal(ctx(next).selected, null, "the selected call went with its callee");
});

test("treeChanged keeps an EXPLORE context whose call still exists", () => {
  const s = run(emptySession(), { type: "select", id: Cm }, { type: "activate", id: exploreId(Cm) });
  const next = changed(s, without("g"));
  assert.deepEqual(next.order, [MAIN, exploreId(Cm)]);
  assert.equal(next.active, exploreId(Cm));
});

test("a selected node that goes Missing is dropped; the rest of the session stays", () => {
  const s = run(emptySession(), { type: "focus", id: "pkg" }, { type: "select", id: "m" }, { type: "expand", ids: ["pkg"] });
  const next = changed(s, without("m"));
  assert.equal(ctx(next).selected, null);
  assert.deepEqual([[...ctx(next).focus], [...openOf(ctx(next))]], [["pkg"], ["pkg"]], "the focus and the open cards are untouched");
});

test("treeChanged drops what the cursor, the highlight and a reveal point at when it is gone", () => {
  const s = run(emptySession(), { type: "cursor", id: "f" }, { type: "highlight", id: "f" }, { type: "reveal", id: "f", canvas: true });
  const next = changed(s, without("f"));
  assert.deepEqual([next.cursor, next.highlight, next.reveal], [null, null, null]);
});

test("open call ids stay open while their keys exist, and go when they do not", () => {
  const s = run(emptySession(), { type: "expand", ids: ["pkg", Cm, Cmg] });
  assert.deepEqual([...openOf(ctx(changed(s, tree)))], ["pkg", Cm, Cmg]);
  assert.deepEqual([...openOf(ctx(changed(s, without("g"))))], ["pkg", Cm], "Cmg names g");
});

// ── the rest of the reducer, as before ────────────────────────────────────────────────────────────────────────────────

test("toggle, expand and collapseAll change only the open nodes", () => {
  const s = run(emptySession(), { type: "toggle", id: "pkg" }, { type: "expand", ids: ["a.py", "pkg"] });
  assert.deepEqual([...openOf(ctx(s))], ["pkg", "a.py"]);
  assert.deepEqual([...openOf(ctx(run(s, { type: "toggle", id: "pkg" })))], ["a.py"]);
  assert.deepEqual([...openOf(ctx(run(s, { type: "collapseAll" })))], []);
  const nothingOpen = emptySession();
  assert.equal(run(nothingOpen, { type: "collapseAll" }), nothingOpen, "nothing to close: the same object");
});

test("reveal opens the ancestors and selects; it asks the canvas only when told to", () => {
  const s = run(emptySession(), { type: "focus", id: "a.py" });
  const shown = run(s, { type: "reveal", id: "f", canvas: true });
  assert.deepEqual([...openOf(ctx(shown))], ["b.py"], "every ancestor of f under the root of what is drawn, not f itself: the project is always open");
  assert.deepEqual(shown.reveal, { context: "main", id: "f", n: shown.seq });
  const inside = run(s, { type: "reveal", id: "m", canvas: false });
  assert.deepEqual([...openOf(ctx(inside))], ["C"], "inside the focus on a.py: only what is between a.py and m");
  assert.equal(inside.reveal, null, "the sidebar's reveal leaves nothing for the canvas");
  assert.equal(run(s, { type: "reveal", id: "gone", canvas: true }), s, "a node that is not in the tree");
});

test("reveal on a call card opens its owner and the call cards above it, selects nothing, and asks the canvas for the card (9.5a)", () => {
  const before = run(emptySession(), { type: "select", id: "g" });
  const shown = run(before, { type: "reveal", id: Cmg, canvas: true });
  assert.deepEqual([...openOf(ctx(shown))], ["b.py", "f", Cm], "the owner's ancestors, the owner, and the call card above the one asked for");
  assert.equal(ctx(shown).selected, "g", "the selection is where it was: selecting a call card would follow it");
  assert.deepEqual(shown.reveal, { context: "main", id: Cmg, n: shown.seq });
  const top = run(emptySession(), { type: "reveal", id: Cm, canvas: true });
  assert.deepEqual([...openOf(ctx(top))], ["b.py", "f"], "a call card of the owner itself: no call card above it");
  assert.equal(run(emptySession(), { type: "reveal", id: callId("gone", ["m"]), canvas: true }).reveal, null, "an owner that is not in the tree: nothing");
  const focused = run(emptySession(), { type: "focus", id: "a.py" }, { type: "reveal", id: Cmg, canvas: true });
  assert.deepEqual(ctx(focused).focus, [], "the owner is outside the focus: the path drops to what contains it, as for a structure card");
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
  assert.equal(ctx(s).selected, "m");
  assert.deepEqual([...ctx(s).focus], []);
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

test("an intent that changes nothing, or names a context that does not exist, returns the same object; arrays that did not move are kept", () => {
  const s = run(emptySession(), { type: "focus", id: "pkg" }, { type: "expand", ids: ["pkg"] });
  assert.equal(run(s, { type: "select", id: "pkg" }), s);
  assert.equal(reduce(s, "nope", { type: "focus", id: "b.py" }, tree, lookup), s);
  const selected = run(s, { type: "select", id: "m" });
  assert.equal(ctx(selected).focus, ctx(s).focus, "only the selection moved: focus is the same array");
  assert.equal(ctx(selected).open, ctx(s).open);
  assert.equal(run(s, { type: "expand", ids: ["pkg"] }), s);
});

test("keepEqual reuses the arrays whose contents did not change", () => {
  const old = run(emptySession(), { type: "focus", id: "pkg" }, { type: "expand", ids: ["pkg", "a.py"] });
  const arrived: Session = JSON.parse(JSON.stringify({ ...old, cursor: "m", seq: old.seq + 1 }));
  assert.notEqual(arrived.contexts.main.focus, old.contexts.main.focus, "a message has brand-new arrays");
  const kept = keepEqual(old, arrived);
  assert.equal(kept.contexts.main.focus, old.contexts.main.focus);
  assert.equal(kept.contexts.main.open, old.contexts.main.open);
  assert.equal(kept.order, old.order);
  assert.equal(kept.cursor, "m");
  const asked = run(old, { type: "reveal", id: "m", canvas: true });
  assert.equal(keepEqual(asked, JSON.parse(JSON.stringify({ ...asked, cursor: "f", seq: asked.seq + 1 }))).reveal, asked.reveal, "the same request is the same object");
  const moved = keepEqual(old, { ...arrived, contexts: { main: { ...arrived.contexts.main, focus: ["pkg", "a.py"] } } });
  assert.deepEqual(moved.contexts.main.focus, ["pkg", "a.py"], "a real change comes through");
});

// ── saving ────────────────────────────────────────────────────────────────────────────────────────────────────────────

const roundTrip = (s: Session) => restoreSession(JSON.parse(JSON.stringify(toSaved(s))));

test("a saved session comes back: the contexts, their order, which one is active; the cursor, highlight and reveal do not", () => {
  const e1 = exploreId(Cm);
  let s = run(emptySession(), { type: "focus", id: "pkg" }, { type: "expand", ids: ["a.py", Cm] }, { type: "select", id: Cm }, { type: "cursor", id: "m" });
  s = runIn(e1, s, { type: "select", id: Cmg }, { type: "toggle", id: "C" });
  s = run(s, { type: "activate", id: exploreId(Cmg) });
  const back = roundTrip(s);
  assert.deepEqual(back.contexts, s.contexts);
  assert.deepEqual(back.order, s.order);
  assert.equal(back.active, s.active);
  assert.deepEqual([back.cursor, back.highlight, back.reveal, back.seq], [null, null, null, 0]);
  assert.deepEqual(sessionProblems(back, tree), []);
});

test("toSaved then restoreSession is the identity on a session with every kind of context", () => {
  const plain = run(emptySession(), { type: "focus", id: "m" });
  assert.deepEqual(roundTrip(plain).contexts, plain.contexts);
  const fresh = emptySession();
  assert.deepEqual(roundTrip(fresh), fresh);
});

const garbage: [string, unknown][] = [
  ["undefined", undefined],
  ["null", null],
  ["a number", 5],
  ["a string", "main"],
  ["an array", []],
  ["an object with no fields", {}],
  ["contexts that is not an object", { contexts: 5, order: ["main"], active: "main" }],
  ["order that is not a list", { contexts: {}, order: "main", active: "main" }],
  ["a MAIN with a missing field", { contexts: { main: { id: "main" } }, order: ["main"], active: "main" }],
  ["no MAIN at all", { contexts: { x: { id: "x", title: "x", kind: "explore", parent: "main", call: Cm, focus: [], selected: null, expanded: [] } }, order: ["x"], active: "x" }],
  ["a MAIN with a parent", { contexts: { main: { id: "main", title: "Main", kind: "main", parent: "x", call: null, focus: [], selected: null, expanded: [] } }, order: ["main"], active: "main" }],
  ["a focus that is not strings", { contexts: { main: { id: "main", title: "Main", kind: "main", parent: null, call: null, focus: [1], selected: null, expanded: [] } }, order: ["main"], active: "main" }],
];
for (const [name, value] of garbage) {
  test(`restoreSession: ${name} gives a clean MAIN`, () => assert.deepEqual(restoreSession(value), emptySession()));
}

test("restoreSession: the old focus key is read once, and only when it is a list of strings", () => {
  assert.deepEqual([...ctx(restoreSession(undefined, ["a", "b"])).focus], ["a", "b"]);
  assert.deepEqual([...ctx(restoreSession(undefined, "nonsense")).focus], []);
  assert.deepEqual([...ctx(restoreSession(5, ["a"])).focus], ["a"], "garbage saved, old key present: the old key");
});

// a saved session with MAIN and one EXPLORE, then one thing wrong with the EXPLORE: MAIN is kept, the EXPLORE is dropped, active is MAIN
const savedWith = (explore: Partial<ContextState> | null, order = ["main", "x"], active = "x") => {
  const base = emptySession().contexts.main;
  const x = explore && { id: "x", title: "m", kind: "explore", parent: "main", call: Cm, focus: ["pkg"], selected: null, expanded: [], ...explore };
  return { contexts: { main: { ...base, focus: ["b.py"], selected: Cm }, ...(x ? { x } : {}) }, order, active };
};
const brokenExplores: [string, ReturnType<typeof savedWith>][] = [
  ["a call that is not a call id", savedWith({ call: "m" })],
  ["no call at all", savedWith({ call: null })],
  ["a parent that is not there", savedWith({ parent: "gone" })],
  ["a parent that comes after it", savedWith({ parent: "x" })],
  ["an unknown kind", savedWith({ kind: "weird" as never })],
  ["a context in order that is not saved", savedWith(null)],
];
for (const [name, saved] of brokenExplores) {
  test(`restoreSession: an EXPLORE with ${name} is dropped; MAIN and what it had stay`, () => {
    const back = restoreSession(saved);
    assert.deepEqual(back.order, [MAIN]);
    assert.equal(back.active, MAIN);
    assert.deepEqual([...ctx(back).focus], ["b.py"], "MAIN is not lost for the sake of a broken panel");
  });
}

test("restoreSession: a saved EXPLORE whose call no longer exists is closed by the first treeChanged (the call trees are not saved, so it is not known earlier)", () => {
  const saved = savedWith({ call: Cg, focus: ["b.py", "g"], selected: "g" });
  (saved.contexts.main as ContextState).selected = Cg;
  const back = restoreSession(saved);
  assert.deepEqual(back.order, [MAIN, "x"], "structurally sound, so it is restored");
  assert.deepEqual(changed(back, without("g")).order, [MAIN]);
});

test("restoreSession: two EXPLORE contexts for the same call keep the first; a duplicate in order is one", () => {
  const saved = savedWith({});
  Object.assign(saved.contexts, { y: { ...(saved.contexts as Record<string, ContextState>).x, id: "y" } });
  assert.deepEqual(restoreSession({ ...saved, order: ["main", "x", "y"] }).order, ["main", "x"]);
  assert.deepEqual(restoreSession({ ...saved, order: ["main", "x", "x"] }).order, ["main", "x"]);
});

test("restoreSession: an active that is not open is MAIN", () => {
  assert.equal(restoreSession(savedWith({}, ["main", "x"], "gone")).active, MAIN);
  assert.equal(restoreSession(savedWith({}, ["main", "x"], "x")).active, "x");
});

// ── callsChanged (05 §5): a new call tree arrived for an owner ────────────────────────────────────────────────────────

/** `reduce` with a different call lookup, and the invariants checked after the step. */
const callsChange = (s: Session, owner: string, trees: Parameters<typeof callLookup>[0]): Session => {
  const next = reduce(s, MAIN, { type: "callsChanged", owner }, tree, callLookup(trees));
  assert.deepEqual(sessionProblems(next, tree), [], "sound after callsChanged");
  return next;
};
const treeOf = (...calls: CallTreeNode[]) => ({ f: { root: call("f", "f", calls) } });
const callAt = (key: string, line: number, children: CallTreeNode[] = []): CallTreeNode => ({ ...call(key, key, children), call_sites: [{ start: { line, character: 0 }, end: { line, character: 3 }, path: "b.py" }] });

test("callsChanged: a call removed from the code while both functions still exist closes its EXPLORE, and a selection on it falls back to the owner function", () => {
  let s = run(emptySession(), { type: "explore", call: Cm, activate: true });
  assert.equal(s.active, exploreId(Cm));
  const next = callsChange(s, "f", treeOf(call("g", "g"))); // f no longer calls m (m and f are both still in the project)
  assert.deepEqual(next.order, [MAIN]);
  assert.equal(next.active, MAIN, "the canvas was on the closed context");
  assert.equal(ctx(next).selected, "f", "the selected call fell back to the function that made it");
  s = next;
  assert.equal(callsChange(s, "f", treeOf(call("g", "g"))), s, "and again: nothing more to drop");
});

test("callsChanged: the same call moved to another line has the same id: nothing closes, nothing changes", () => {
  const before = treeOf(callAt("m", 3), callAt("g", 5));
  const s = run(emptySession(), { type: "select", id: Cm }, { type: "expand", ids: [Cm] });
  assert.equal(callsChange(s, "f", before), s);
  assert.equal(callsChange(s, "f", treeOf(callAt("m", 40), callAt("g", 5))), s, "m is now called on line 41: the same id");
});

test("callsChanged: a call inserted above another leaves the other's id as it was", () => {
  const s = run(emptySession(), { type: "select", id: Cg }, { type: "expand", ids: [Cg] });
  assert.equal(callsChange(s, "f", treeOf(callAt("m", 1), callAt("g", 5))), s);
  assert.equal(callsChange(s, "f", treeOf(callAt("brand_new", 1), callAt("m", 2), callAt("g", 6))), s, "a new call above g");
});

test("callsChanged: expanded call ids that are gone are dropped, the others kept (structure keys are not touched)", () => {
  const s = run(emptySession(), { type: "expand", ids: ["pkg", Cm, Cmg, Cg, "f"] });
  const next = callsChange(s, "f", treeOf(call("g", "g"))); // m is gone, and Cmg with it
  assert.deepEqual([...openOf(ctx(next))], ["pkg", Cg, "f"]);
});

test("callsChanged: a call deeper than an EXPLORE goes: that context stays, its selection falls back, its child closes", () => {
  const e1 = exploreId(Cm);
  let s = run(emptySession(), { type: "select", id: Cm });
  s = runIn(e1, s, { type: "select", id: Cmg });
  assert.deepEqual(s.order, [MAIN, e1, exploreId(Cmg)]);
  const next = callsChange(s, "f", treeOf(call("m", "m"), call("g", "g"))); // m still calls nothing: Cmg is gone, Cm is not
  assert.deepEqual(next.order, [MAIN, e1]);
  assert.equal(ctx(next, e1).selected, "f");
});

test("callsChanged: the owner's tree failed to load (or is not held): that is unknown, not gone: nothing is closed", () => {
  const s = run(emptySession(), { type: "select", id: Cm }, { type: "expand", ids: [Cm, Cmg] });
  const none = callsChange(s, "f", {}); // no tree for f
  assert.equal(none, s, "the same object: nothing was touched");
  assert.equal(none.order.length, 2);
});

test("callsChanged for one owner leaves the call ids of another owner alone", () => {
  const s = run(emptySession(), { type: "select", id: Cm }, { type: "expand", ids: [Cm] });
  assert.equal(callsChange(s, "m", treeOf(call("g", "g"))), s, "the tree that arrived is m's; Cm belongs to f");
});

// ── 8.4b / 03 §2.1: what is open belongs to the focus root ─────────────────────────────────────────────────────────

/** The set of each root, by root ("" = no focus), as plain lists. */
const sets = (s: Session, id: string = MAIN) => Object.fromEntries(Object.entries(ctx(s, id).open).map(([root, ids]) => [root, [...ids]]));

test("focus on X for the first time: the open ids under X carry in, the others stay with the root they were opened in", () => {
  const s = run(emptySession(), { type: "expand", ids: ["pkg", "a.py", "C", "b.py"] }, { type: "focus", id: "a.py" });
  assert.deepEqual([...openOf(ctx(s))], ["a.py", "C"], "a.py and C are under a.py; pkg and b.py are not");
  assert.deepEqual(sets(s)[""], ["pkg", "a.py", "C", "b.py"], "the project's own set is as it was");
});

test("what is opened inside a focus stays inside it: clearing the focus gives the project as it was left", () => {
  let s = run(emptySession(), { type: "expand", ids: ["pkg"] });
  s = run(s, { type: "focus", id: "a.py" }, { type: "expand", ids: ["C", "a.py"] }, { type: "focusJump", depth: 0 });
  assert.deepEqual([...openOf(ctx(s))], ["pkg"], "the opening done inside a.py did not come out with it");
  s = run(s, { type: "focus", id: "a.py" });
  assert.deepEqual([...openOf(ctx(s))], ["C", "a.py"], "focus on a.py again: its set as it was left, not carried in again");
});

test("Back and a crumb go to the parent root's own set, as it was left", () => {
  let s = run(emptySession(), { type: "focus", id: "a.py" }, { type: "expand", ids: ["a.py"] }, { type: "focus", id: "C" }, { type: "expand", ids: ["C", "m"] });
  assert.deepEqual([...openOf(ctx(s))], ["C", "m"]);
  s = run(s, { type: "focusBack" });
  assert.deepEqual([ctx(s).focus.at(-1), [...openOf(ctx(s))]], ["a.py", ["a.py"]], "Back: a.py's set");
  s = run(s, { type: "focus", id: "C" }, { type: "focusJump", depth: 1 });
  assert.deepEqual([ctx(s).focus.at(-1), [...openOf(ctx(s))]], ["pkg", ["C", "m"]], "a crumb to a root that never had a set: what was open in the root it leaves, under pkg, carries in");
});

test("toggle, expand and Collapse All change the current root's set only; the others keep theirs", () => {
  let s = run(emptySession(), { type: "expand", ids: ["pkg", "b.py"] }, { type: "focus", id: "a.py" }, { type: "expand", ids: ["C"] });
  s = run(s, { type: "toggle", id: "a.py" });
  assert.deepEqual(sets(s), { "": ["pkg", "b.py"], "a.py": ["C", "a.py"] });
  s = run(s, { type: "collapseAll" });
  assert.deepEqual(sets(s), { "": ["pkg", "b.py"], "a.py": [] }, "only the current root empties");
});

test("Reveal while focused: only the current root's set grows, and with the ancestors between the root and the node", () => {
  const before = run(emptySession(), { type: "expand", ids: ["b.py"] }, { type: "focus", id: "pkg" });
  const shown = run(before, { type: "reveal", id: "m", canvas: true });
  assert.deepEqual([ctx(shown).focus.at(-1), [...openOf(ctx(shown))]], ["pkg", ["a.py", "C"]], "a.py and C: between pkg and m; pkg is the root, always open");
  assert.deepEqual(sets(shown)[""], ["b.py"], "the project's set did not grow with the ancestors");
  // revealing outside the focus drops steps, and the ancestors go into the root it lands on
  const far = run(before, { type: "reveal", id: "f", canvas: true });
  assert.deepEqual([[...ctx(far).focus], [...openOf(ctx(far))]], [[], ["b.py"]]);
});

test("clearing the focus after opening many things and revealing deep nodes draws the project as it was, nothing more", () => {
  let s = run(emptySession(), { type: "expand", ids: ["pkg"] });
  s = run(s, { type: "focus", id: "m" }, { type: "reveal", id: "m", canvas: true }, { type: "expand", ids: ["a.py", "C"] }, { type: "reveal", id: "f", canvas: true });
  s = run(s, { type: "focusJump", depth: 0 });
  assert.deepEqual(sets(s)[""], ["pkg", "b.py"], "pkg was open before; b.py is what the reveal of f opened at the project; a.py and C, opened inside the focus, are not here");
  assert.deepEqual([...openOf(ctx(s))], ["pkg", "b.py"]);
});

test("a 21st root drops the one used longest ago; a root used again moves to the end", () => {
  const files = Array.from({ length: 22 }, (_, i) => node(`f${i}.py`, "file", [node(`f${i}`, "function")]));
  const wide = node("/", "folder", files);
  let s = emptySession();
  const go = (...intents: Intent[]) => {
    for (const intent of intents) {
      s = reduce(s, MAIN, intent, wide, lookup);
      assert.deepEqual(sessionProblems(s, wide), [], `after ${JSON.stringify(intent)}`);
    }
  };
  for (let i = 0; i < 20; i++) go({ type: "focus", id: `f${i}.py` }, { type: "expand", ids: [`f${i}`] });
  assert.equal(ctx(s).recent.length, 20);
  go({ type: "focus", id: "f0.py" }); // f0 is used again: it is now the newest
  go({ type: "focus", id: "f20.py" }); // the 21st root (its set is new): f1 is the one used longest ago
  assert.equal(ctx(s).recent.length, 20);
  assert.ok(!("f1.py" in ctx(s).open) && "f0.py" in ctx(s).open && "f20.py" in ctx(s).open, `kept ${Object.keys(ctx(s).open)}`);
  assert.deepEqual(ctx(s).recent.slice(-2), ["f0.py", "f20.py"]);
  go({ type: "focus", id: "f1.py" }); // a dropped root comes back empty (carried in from the root it left: nothing under it)
  assert.deepEqual([...openOf(ctx(s))], []);
});

test("the project's root keeps its set however many roots are visited (03 §2.1)", () => {
  const files = Array.from({ length: 25 }, (_, i) => node(`f${i}.py`, "file", [node(`f${i}`, "function")]));
  const wide = node("/", "folder", [node("pkg", "folder", files)]);
  let s = emptySession();
  const go = (...intents: Intent[]) => {
    for (const intent of intents) {
      s = reduce(s, MAIN, intent, wide, lookup);
      assert.deepEqual(sessionProblems(s, wide), [], `after ${JSON.stringify(intent)}`);
    }
  };
  go({ type: "expand", ids: ["pkg", "f3.py"] }); // opened at "": the oldest set from here on
  for (let i = 0; i < 25; i++) {
    go({ type: "focus", id: `f${i}.py` }, { type: "expand", ids: [`f${i}`] });
    assert.deepEqual(ctx(s).open[""], ["pkg", "f3.py"], `the project's set after visiting f${i}.py`);
    assert.ok(ctx(s).recent.length <= 20, `${ctx(s).recent.length} roots kept`);
  }
  assert.equal(ctx(s).recent.length, 20);
  assert.ok(ctx(s).recent.includes("") && !("f0.py" in ctx(s).open) && "f24.py" in ctx(s).open, `kept ${Object.keys(ctx(s).open)}`);
  go({ type: "focusJump", depth: 0 });
  assert.deepEqual([[...ctx(s).focus], [...openOf(ctx(s))]], [[], ["pkg", "f3.py"]], "clearing the focus gives the project back as it was left");
});

test("the code changed: ids that are gone leave every set, and a root that is gone loses its set", () => {
  let s = run(emptySession(), { type: "expand", ids: ["pkg", "b.py", "g"] }, { type: "focus", id: "f" });
  s = run(s, { type: "focusBack" }, { type: "focus", id: "g" }, { type: "expand", ids: ["g"] });
  assert.deepEqual(Object.keys(sets(s)).sort(), ["", "f", "g", "b.py"].sort());
  const next = changed(s, without("g"));
  assert.deepEqual(Object.keys(sets(next)).sort(), ["", "b.py", "f"].sort(), "g is gone, and so is its root");
  assert.deepEqual(sets(next)[""], ["pkg", "b.py"], "g left the project's set too");
  assert.deepEqual([...ctx(next).recent].sort(), Object.keys(ctx(next).open).sort());
});

test("a session saved before 8.4b (one `expanded` list) becomes the set of the root it was saved at", () => {
  const old = { contexts: { main: { id: "main", title: "Main", kind: "main", parent: null, call: null, focus: ["pkg", "a.py"], selected: null, expanded: ["a.py", "C"] } }, order: ["main"], active: "main" };
  const back = restoreSession(old);
  assert.deepEqual(sets(back), { "a.py": ["a.py", "C"] });
  assert.deepEqual([...ctx(back).recent], ["a.py"]);
  assert.deepEqual(sessionProblems(back, tree), []);
  assert.deepEqual(sets(restoreSession({ ...old, contexts: { main: { ...old.contexts.main, expanded: [] } } })), {}, "nothing open: no set");
});

test("a saved session round-trips its sets and their order; a `recent` that does not match is rebuilt", () => {
  const s = run(emptySession(), { type: "expand", ids: ["pkg"] }, { type: "focus", id: "a.py" }, { type: "expand", ids: ["C"] });
  const back = restoreSession(JSON.parse(JSON.stringify(toSaved(s))));
  assert.deepEqual([sets(back), [...ctx(back).recent]], [sets(s), [...ctx(s).recent]]);
  const broken = JSON.parse(JSON.stringify(toSaved(s)));
  broken.contexts.main.recent = ["nonsense"];
  assert.deepEqual([...ctx(restoreSession(broken)).recent].sort(), Object.keys(ctx(s).open).sort());
});

test("keepEqual keeps each set that did not change, and the whole record when none did", () => {
  const old = run(emptySession(), { type: "expand", ids: ["pkg"] }, { type: "focus", id: "a.py" }, { type: "expand", ids: ["C"] });
  const arrived: Session = JSON.parse(JSON.stringify({ ...old, seq: old.seq + 1 }));
  const kept = keepEqual(old, arrived);
  assert.equal(kept.contexts.main.open, old.contexts.main.open);
  assert.equal(kept.contexts.main.recent, old.contexts.main.recent);
  const moved: Session = JSON.parse(JSON.stringify({ ...old, contexts: { main: { ...old.contexts.main, open: { ...old.contexts.main.open, "a.py": ["C", "m"] } } } }));
  const partly = keepEqual(old, moved);
  assert.equal(partly.contexts.main.open[""], old.contexts.main.open[""], "the set that did not change is the same array");
  assert.notEqual(partly.contexts.main.open, old.contexts.main.open);
});
