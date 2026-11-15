// What the canvas's doc panel is about (docs/16-ui-redesign/03-canvas.md §8): the active context's selection. Pure: no `vscode`, no React.

import { type CallFacts, type CallId, isCallId, linesText } from "./calls.ts";
import type { TreeNode } from "./messages";
import type { Session } from "./session.ts";
import { findNode } from "./tree.ts";

export interface DocSubject {
  /** the node whose docs are shown; null: nothing to show docs for */
  node: TreeNode | null;
  /** for a selected call: who calls it, and at which lines (1-based) */
  calledBy: { callerName: string; lines: number[] } | null;
}

/**
 * The active context's selected node; a selected call is its callee, with who calls it. Nothing selected (or a selection that is no longer in the tree):
 * the focused node. A call whose callee the project does not know, or whose tree is not held, has no docs to show: not the focused node's, which would be wrong.
 * `facts` says what a call id is.
 */
export function docSubject(session: Pick<Session, "contexts" | "active">, tree: TreeNode, facts: (call: CallId) => CallFacts | null): DocSubject {
  const c = session.contexts[session.active];
  if (!c) return { node: null, calledBy: null };
  if (c.selected !== null && isCallId(c.selected)) {
    const found = facts(c.selected);
    const node = found?.callee.key != null ? findNode(tree, found.callee.key) ?? null : null;
    return found && node ? { node, calledBy: { callerName: found.callerName, lines: found.lines } } : { node: null, calledBy: null };
  }
  const selected = c.selected !== null ? findNode(tree, c.selected) : undefined;
  const focused = c.focus.length > 0 ? findNode(tree, c.focus[c.focus.length - 1]) : undefined;
  return { node: selected ?? focused ?? null, calledBy: null };
}

/** "called by checkout_order · line 16", "called by checkout_order · lines 16, 31". */
export const calledByText = (by: NonNullable<DocSubject["calledBy"]>): string => `called by ${by.callerName} · ${linesText(by.lines)}`;
