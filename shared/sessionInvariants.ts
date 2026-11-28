// What must be true of every session `reduce` returns (docs/16-ui-redesign/03-canvas.md §3–§4). Used by the tests: they run it after every step,
// so a rule broken in one intent shows up in the test of any other. Returns what is wrong, in words; an empty list means the session is sound.

import type { TreeNode } from "./messages";
import { callExists, isCallId } from "./calls.ts";
import { MAIN, MAX_ROOTS, type Session, tourContextId } from "./session.ts";
import { chainedPrefix, findNode } from "./tree.ts";

export function sessionProblems(s: Session, tree: TreeNode): string[] {
  const problems: string[] = [];
  const bad = (text: string) => problems.push(text);
  const ids = Object.keys(s.contexts);

  if (!(s.active in s.contexts)) bad(`active "${s.active}" is not an open context`);
  if (new Set(s.order).size !== s.order.length) bad("order lists a context twice");
  if (s.order.length !== ids.length || !s.order.every((id) => id in s.contexts)) bad("order and contexts do not hold the same contexts");
  if (s.order[0] !== MAIN) bad("MAIN is not first in order");
  if (s.contexts[s.active]?.preview) bad(`the active context ${s.active} is a preview: being active ends a preview`);

  if (s.tourList.view !== "all" && s.tourList.view !== "onSelection") bad(`tourList.view "${String(s.tourList.view)}" is neither "all" nor "onSelection"`);

  const calls = new Set<string>();
  const tours: string[] = [];
  s.order.forEach((id, at) => {
    const c = s.contexts[id];
    if (!c) return;
    if (c.id !== id) bad(`${id}: its id says ${c.id}`);
    if (c.kind === "main") {
      if (id !== MAIN || c.parent !== null || c.call !== null || c.tour !== null) bad(`${id}: MAIN has no parent, no call and no tour, and it is the only one`);
      if (c.preview) bad(`${id}: MAIN is never a preview`);
    } else if (c.kind === "tour") {
      // a tour context: MAIN's child, no call, one tour; one per window; the place of the tour is not in it
      if (c.tour === null || c.id !== tourContextId(c.tour)) bad(`${id}: a TOUR context names its tour, and its id says it`);
      if (c.parent !== MAIN || c.call !== null) bad(`${id}: a TOUR context belongs to MAIN and has no call`);
      if (c.preview) bad(`${id}: a TOUR context is never a preview`);
      tours.push(id);
    } else {
      if (c.tour !== null) bad(`${id}: an EXPLORE context has no tour`);
      if (c.parent === null || c.call === null) bad(`${id}: an EXPLORE context has a parent and a call`);
      else {
        const parentAt = s.order.indexOf(c.parent);
        if (!(c.parent in s.contexts)) bad(`${id}: its parent ${c.parent} is not open`);
        else if (parentAt >= at) bad(`${id}: its parent ${c.parent} does not come before it in order`);
        else if (s.contexts[c.parent].selected !== c.call) bad(`${id}: its parent has not selected its call (an EXPLORE panel belongs to the selected call)`);
        if (!callExists(tree, c.call)) bad(`${id}: its call does not exist in the tree`);
        if (calls.has(c.call)) bad(`${id}: a second EXPLORE context for the same call`);
        calls.add(c.call);
      }
    }
    if (chainedPrefix(tree, c.focus).length !== c.focus.length) bad(`${id}: focus [${c.focus.join(", ")}] is not a path in the tree`);
    if (c.selected !== null && !(isCallId(c.selected) ? callExists(tree, c.selected) : findNode(tree, c.selected) !== undefined)) bad(`${id}: selected "${c.selected}" does not exist`);
    // what is open (03 §2.1): per root, only what exists, only roots that exist, and `recent` lists exactly those roots, at most MAX_ROOTS of them
    const roots = Object.keys(c.open);
    if (roots.length > MAX_ROOTS) bad(`${id}: ${roots.length} roots keep an open set, at most ${MAX_ROOTS}`);
    if (c.recent.length !== roots.length || !roots.every((r) => c.recent.includes(r)) || new Set(c.recent).size !== c.recent.length) bad(`${id}: recent [${c.recent.join(", ")}] is not the roots that have a set [${roots.join(", ")}]`);
    for (const root of roots) {
      if (root !== "" && findNode(tree, root) === undefined) bad(`${id}: an open set for the root "${root}", which does not exist`);
      for (const open of c.open[root]) if (!(isCallId(open) ? callExists(tree, open) : findNode(tree, open) !== undefined)) bad(`${id}: "${open}" is open (root "${root}") and does not exist`);
    }
  });
  if (tours.length > 1) bad(`${tours.length} TOUR contexts are open (${tours.join(", ")}): there is one per window`);
  return problems;
}
