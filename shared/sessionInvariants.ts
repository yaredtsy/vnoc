// What must be true of every session `reduce` returns (docs/16-ui-redesign/03-canvas.md §3–§4). Used by the tests: they run it after every step,
// so a rule broken in one intent shows up in the test of any other. Returns what is wrong, in words; an empty list means the session is sound.

import type { TreeNode } from "./messages";
import { callExists, isCallId } from "./calls.ts";
import { MAIN, type Session } from "./session.ts";
import { chainedPrefix, findNode } from "./tree.ts";

export function sessionProblems(s: Session, tree: TreeNode): string[] {
  const problems: string[] = [];
  const bad = (text: string) => problems.push(text);
  const ids = Object.keys(s.contexts);

  if (!(s.active in s.contexts)) bad(`active "${s.active}" is not an open context`);
  if (new Set(s.order).size !== s.order.length) bad("order lists a context twice");
  if (s.order.length !== ids.length || !s.order.every((id) => id in s.contexts)) bad("order and contexts do not hold the same contexts");
  if (s.order[0] !== MAIN) bad("MAIN is not first in order");

  const calls = new Set<string>();
  s.order.forEach((id, at) => {
    const c = s.contexts[id];
    if (!c) return;
    if (c.id !== id) bad(`${id}: its id says ${c.id}`);
    if (c.kind === "main") {
      if (id !== MAIN || c.parent !== null || c.call !== null) bad(`${id}: MAIN has no parent and no call, and it is the only one`);
    } else {
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
  });
  return problems;
}
