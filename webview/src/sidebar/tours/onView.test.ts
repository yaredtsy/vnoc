import assert from "node:assert/strict";
import { test } from "node:test";

import type { TreeNode } from "@vnoc/shared/messages.ts";
import { type ContextState, MAIN, emptySession } from "@vnoc/shared/session.ts";
import type { TourRow } from "@vnoc/shared/tours.ts";
import { ASK_FOR_ONE, copyRequest, emptyOn, lineTwo, offChipText, offChipTip, opensAt, selectionOf, showAll, stateWordOf, toursChipText, toursChipTip, viewButtons, viewKey, viewOf } from "./onView.ts";

const row = (on?: TourRow["on"]): TourRow => ({ id: "t-1", folder: "private", readable: true, quiet: false, root: null, place: null, offers: null, words: "finished · 6 stops · today", state_word: "finished", ...(on ? { on } : {}) });
const node = (id: string, children: TreeNode[] = []): TreeNode => ({ id, name: id, kind: "function", children, docs: 0 }) as unknown as TreeNode;
const tree = node("/", [node("a.py", [node("a.py::f#1"), node("a.py::g#1")])]);
const noFacts = () => null;

test("the view is the saved one only when there is a selection to follow; with none it is All", () => {
  assert.equal(viewOf("onSelection", "a.py::f#1"), "on");
  assert.equal(viewOf("onSelection", null), "all");
  assert.equal(viewOf("all", "a.py::f#1"), "all");
});

test("the buttons: All <n> and On <name> <n>, the counts being the lengths of the core's two lists; with nothing selected On selection is disabled with its tooltip", () => {
  const [all, on] = viewButtons("all", 5, { name: "reserve_stock" }, 3);
  assert.deepEqual([all.text, on.text, on.disabled, on.tip, all.pressed, on.pressed], ["All 5", "On reserve_stock 3", false, null, true, false]);
  assert.equal(viewButtons("on", 5, { name: "reserve_stock" }, 3)[1].pressed, true);
  assert.equal(viewButtons("all", 5, { name: "reserve_stock" }, null)[1].text, "On reserve_stock", "no count until the core has answered");
  const [, none] = viewButtons("all", 5, null, null);
  assert.deepEqual([none.text, none.disabled, none.tip], ["On selection", true, "Select a node to see its tours"]);
});

test("← and → on the buttons switch the view, never to a disabled one", () => {
  assert.equal(viewKey("ArrowRight", "all", true), "on");
  assert.equal(viewKey("ArrowRight", "all", false), null);
  assert.equal(viewKey("ArrowLeft", "on", true), "all");
  assert.equal(viewKey("ArrowLeft", "all", true), null);
  assert.equal(viewKey("ArrowRight", "on", true), null);
  assert.equal(viewKey("Enter", "all", true), null);
});

test("line 2 is the core's on.words in the second view and the row's own words in All", () => {
  const on = row({ how: "stops", stops: [3], inside: 0, first: "s3", of: 10, words: "stop 3 of 10" });
  assert.equal(lineTwo(on, "on"), "stop 3 of 10");
  assert.equal(lineTwo(on, "all"), "finished · 6 stops · today");
  assert.equal(lineTwo(row(), "on"), "finished · 6 stops · today", "a row with no `on` says its own words");
});

test("the On view keeps the state word at the right of line 1; All has it in line 2 and draws none there", () => {
  assert.equal(stateWordOf(row(), "on"), "finished");
  assert.equal(stateWordOf(row(), "all"), null);
  assert.equal(stateWordOf({ ...row(), state_word: "" }, "on"), null, "a tour that cannot be read has none");
  assert.equal(stateWordOf({ ...row(), state_word: "stop 4" }, "on"), "stop 4");
});

test("a click opens at the cover for starts, else at the stop the core named; in All, where it was left", () => {
  assert.equal(opensAt(row({ how: "starts", stops: [1], inside: 0, first: "s1", of: 6, words: "starts here" }), "on"), "cover");
  assert.equal(opensAt(row({ how: "stops", stops: [3], inside: 0, first: "s3", of: 10, words: "stop 3 of 10" }), "on"), "s3");
  assert.equal(opensAt(row({ how: "inside", stops: [], inside: 4, first: "s2", of: 10, words: "4 stops inside" }), "on"), "s2");
  assert.equal(opensAt(row({ how: "stops", stops: [3], inside: 0, first: "s3", of: 10, words: "stop 3 of 10" }), "all"), null);
});

test("the empty state and its button say the core's counts", () => {
  assert.equal(emptyOn("gateway.py"), "No tour goes through gateway.py.");
  assert.equal(showAll(5), "Show All 5");
});

test("the node's selection: the active context's, a call as its callee; none for nothing, the root, a node the tree lacks, an unknown call, a tour context", () => {
  const withSelected = (selected: string | null, kind: ContextState["kind"] = "main") => {
    const s = emptySession();
    return { ...s, contexts: { [MAIN]: { ...s.contexts[MAIN], selected, kind } } };
  };
  assert.equal(selectionOf(withSelected("a.py::f#1"), tree, noFacts), "a.py::f#1");
  assert.equal(selectionOf(withSelected(null), tree, noFacts), null);
  assert.equal(selectionOf(withSelected("/"), tree, noFacts), null, "the root is every tour's");
  assert.equal(selectionOf(withSelected("gone.py"), tree, noFacts), null);
  assert.equal(selectionOf(withSelected("a.py::f#1", "tour"), tree, noFacts), null, "a tour shows its route");
  assert.equal(selectionOf(withSelected("a.py::f#1"), null, noFacts), null);
  const call = "call:a.py::f#1/a.py::g#1";
  const facts = () => ({ callee: { key: "a.py::g#1" } }) as never;
  assert.equal(selectionOf(withSelected(call), tree, facts), "a.py::g#1");
  assert.equal(selectionOf(withSelected(call), tree, () => ({ callee: { key: null } }) as never), null);
});

test("▶ N and +N: their text and tooltips, one and many", () => {
  assert.equal(toursChipText(3), "▶ 3");
  assert.equal(toursChipTip(3), "on 3 tours: click to see them");
  assert.equal(toursChipTip(1), "on 1 tour: click to see it");
  assert.equal(offChipText(2), "+2");
  assert.equal(offChipTip(2), "2 calls are not on this tour");
  assert.equal(offChipTip(1), "1 call is not on this tour");
});

test("the empty states' buttons that ask for a tour: Ask for One; Copy a Request for the selection, or none", () => {
  assert.equal(ASK_FOR_ONE, "Ask for One");
  assert.equal(copyRequest("reserve_stock"), "Copy a Request for reserve_stock");
  assert.equal(copyRequest(null), "Copy a Request");
});
