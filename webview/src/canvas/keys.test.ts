import assert from "node:assert/strict";
import { test } from "node:test";

import { type Calls, callId } from "@vnoc/shared/calls.ts";
import type { CallSite, CallTreeNode, Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { type KeyContext, canvasChord, canvasKey, canvasViewKey } from "./keys.ts";
import { layout } from "./layout.ts";

const n = (id: string, kind: Kind, children: TreeNode[] = []): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
// / ├ a.py ├ f      / ├ b.py     (a.py is open, b.py has no children)
const root = n("/", "folder", [n("a.py", "file", [n("f", "function")]), n("b.py", "file")]);
const laid = layout(root, new Set(["a.py"])); // rows: / a.py f b.py
const closed = layout(root, new Set()); // rows: / a.py b.py

test("up and down move through the rows, and stop at the ends", () => {
  assert.deepEqual(canvasKey("ArrowDown", laid, root, "a.py"), { type: "select", id: "f" });
  assert.deepEqual(canvasKey("ArrowUp", laid, root, "b.py"), { type: "select", id: "f" });
  assert.deepEqual(canvasKey("ArrowUp", laid, root, "/"), { type: "select", id: "/" });
  assert.deepEqual(canvasKey("ArrowDown", laid, root, "b.py"), { type: "select", id: "b.py" });
  assert.deepEqual(canvasKey("ArrowDown", laid, root, null), { type: "select", id: "/" }, "nothing selected: the first row");
});

test("left selects the parent; right opens a closed card, else goes to the first child", () => {
  assert.deepEqual(canvasKey("ArrowLeft", laid, root, "f"), { type: "select", id: "a.py" });
  assert.equal(canvasKey("ArrowLeft", laid, root, "/"), null, "the root has no parent");
  assert.deepEqual(canvasKey("ArrowRight", closed, root, "a.py"), { type: "expand", ids: ["a.py"] });
  assert.deepEqual(canvasKey("ArrowRight", laid, root, "a.py"), { type: "select", id: "f" });
  assert.equal(canvasKey("ArrowRight", laid, root, "b.py"), null, "a leaf has nothing to open");
});

test("enter focuses, space opens and closes, escape clears the selection", () => {
  assert.deepEqual(canvasKey("Enter", laid, root, "a.py"), { type: "focus", id: "a.py" });
  assert.deepEqual(canvasKey(" ", laid, root, "a.py"), { type: "toggle", id: "a.py" });
  assert.equal(canvasKey(" ", laid, root, "b.py"), null);
  assert.deepEqual(canvasKey("Escape", laid, root, "a.py"), { type: "select", id: null });
  assert.equal(canvasKey("Escape", laid, root, null), null);
  assert.equal(canvasKey("x", laid, root, "a.py"), null);
});

// a function whose calls a probe counted has a chevron: the keys open it like any card with something under it
const withCalls = n("/", "folder", [n("a.py", "file", [n("f", "function"), n("g", "function")])]);
const probed = (open: string[]) => layout(withCalls, new Set(["a.py", ...open]), new Set(), (key) => (key === "f" ? { state: "probed", count: 2 } : { state: "unknown" })); // rows: / a.py f g

test("→ opens a function that has calls, Space toggles it, and a function with none does nothing", () => {
  assert.deepEqual(canvasKey("ArrowRight", probed([]), withCalls, "f"), { type: "expand", ids: ["f"] });
  assert.deepEqual(canvasKey(" ", probed([]), withCalls, "f"), { type: "toggle", id: "f" });
  assert.equal(canvasKey("ArrowRight", probed([]), withCalls, "g"), null, "g has no calls to open");
  assert.equal(canvasKey(" ", probed([]), withCalls, "g"), null);
});

test("→ on an open function whose calls are not drawn as cards goes nowhere: the card below it is not its child", () => {
  assert.equal(canvasKey("ArrowRight", probed(["f"]), withCalls, "f"), null);
});

// ── call cards (03 §7) ──────────────────────────────────────────────────────────────────────────────────────────────

const site = (line: number, path: string | null = "a.py"): CallSite => ({ start: { line: line - 1, character: 0 }, end: { line: line - 1, character: 3 }, path });
const callNode = (key: string | null, name: string, sites: CallSite[]): CallTreeNode => ({ key, name, qualname: name, path: key && "a.py", kind: "function", call_count: sites.length, call_sites: sites, marks: [], children: [] });
const proj = n("/", "folder", [n("a.py", "file", [n("f", "function"), n("g", "function")])]);
const under = (...children: CallTreeNode[]) => layout(proj, new Set(["a.py", "f"]), new Set(), (id): Calls => (id === "f" ? { state: "known", children } : { state: "unknown" })); // rows: / a.py f <calls> g
const C = (step: string) => callId("f", [step]);

test("Enter on a structure card focuses it; on a call card it explores, activated; on an unknown callee it does nothing", () => {
  const l = under(callNode("g", "g", [site(5)]), callNode(null, "lib", [site(6)]));
  assert.deepEqual(canvasKey("Enter", l, proj, "f"), { type: "focus", id: "f" });
  assert.deepEqual(canvasKey("Enter", l, proj, C("g")), { type: "explore", call: C("g"), activate: true });
  assert.equal(canvasKey("Enter", l, proj, C("?::lib")), null);
});

test("← on a call card selects the card it hangs from; → on an open card goes to its first call card", () => {
  const l = under(callNode("g", "g", [site(5)]));
  assert.deepEqual(canvasKey("ArrowLeft", l, proj, C("g")), { type: "select", id: "f" });
  assert.deepEqual(canvasKey("ArrowLeft", l, proj, "f"), { type: "select", id: "a.py" });
  assert.deepEqual(canvasKey("ArrowDown", l, proj, "f"), { type: "select", id: C("g") }, "↓ goes through the rows, call cards included");
});

test("Ctrl/Cmd+Enter: Go to Code on a structure card, none on a folder", () => {
  const l = under(callNode("g", "g", [site(5)]));
  assert.deepEqual(canvasChord("Enter", l, "f"), { type: "goToCode", key: "f" });
  assert.equal(canvasChord("Enter", l, "/"), null);
  assert.equal(canvasChord("Enter", l, null), null);
  assert.equal(canvasChord("a", l, "f"), null);
});

test("Ctrl/Cmd+Enter on a call card: one site goes there, several are the list, no file or an unknown callee has nowhere to go", () => {
  const one = under(callNode("g", "g", [site(5)]));
  assert.deepEqual(canvasChord("Enter", one, C("g")), { type: "goToCallSite", sites: [site(5)] });
  const many = under(callNode("g", "g", [site(5), site(9), site(12)]));
  const chord = canvasChord("Enter", many, C("g"));
  assert.equal(chord?.type === "goToCallSite" && chord.sites.length, 3);
  const noFile = under(callNode("g", "g", [site(5, null)]));
  assert.equal(canvasChord("Enter", noFile, C("g")), null, "the driver said no file");
  const mixed = under(callNode("g", "g", [site(5, null), site(7)]));
  assert.deepEqual(canvasChord("Enter", mixed, C("g")), { type: "goToCallSite", sites: [site(7)] }, "only the sites that name a file");
});

test("F is Fit View, in either case, and no other key is", () => {
  const table: [string, "fitView" | null][] = [
    ["f", "fitView"],
    ["F", "fitView"],
    ["g", null],
    ["ff", null],
    ["Enter", null],
    [" ", null],
    ["Escape", null],
    ["ArrowDown", null],
    ["", null],
  ];
  for (const [key, want] of table) assert.equal(canvasViewKey(key), want, JSON.stringify(key));
});

test("F is Fit View wherever the focus is, except where a key is typing, a chord or a link's", () => {
  const ctx = (c: Partial<KeyContext>): KeyContext => ({ inField: false, modified: false, onLink: false, ...c });
  const table: [string, Partial<KeyContext>, "fitView" | null][] = [
    ["nothing focused, or a card or a button (a chevron just clicked): Space and Enter stay the button's, F is not", {}, "fitView"],
    ["in a text field or a code card: it is a letter", { inField: true }, null],
    ["with Ctrl, Cmd or Alt held: a chord", { modified: true }, null],
    ["on a link", { onLink: true }, null],
    ["in a field with Cmd held", { inField: true, modified: true }, null],
  ];
  for (const [what, c, want] of table) assert.equal(canvasViewKey("f", ctx(c)), want, what);
  assert.equal(canvasViewKey("F", ctx({})), "fitView", "Shift or Caps Lock");
  assert.equal(canvasViewKey("x", ctx({})), null);
});

test("F is not one of the keys that change the session", () => {
  assert.equal(canvasKey("f", laid, root, "a.py"), null);
  assert.equal(canvasKey("F", laid, root, null), null);
});
