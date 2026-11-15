// The rows the STRUCTURE view draws (docs/16-ui-redesign/06-sidebar.md §2, 07 §1): the structure, flattened to one list in reading order, so a long tree is a
// virtual list. Pure, so the rule is tested; the page only draws what it is given.

import type { TreeNode } from "@vnoc/shared/messages.ts";
import { findNode } from "@vnoc/shared/tree.ts";

export interface FlatRow {
  id: string;
  node: TreeNode;
  /** 0 for a top row */
  depth: number;
  /** it has children in the structure (never calls: the sidebar has no call rows, 07 §2) */
  hasChildren: boolean;
  open: boolean;
  /** the row it hangs from; null for a top row */
  parent: string | null;
  /** the row is the focused node: the top of the list, and open whatever the open set says */
  focusRoot: boolean;
  /** a function inside a class: its icon is a method's */
  inClass: boolean;
  /** 1-based place among its siblings, and how many there are (the screen reader says "3 of 12") */
  posInSet: number;
  setSize: number;
}

interface Item {
  node: TreeNode;
  depth: number;
  parent: string | null;
  posInSet: number;
  setSize: number;
  focusRoot: boolean;
  inClass: boolean;
}

/**
 * With no focus the rows are the project's top level (the opened folder itself is not a row). With a focus they are the focused node alone at the top,
 * always open, with what is under it. A node is open when it has children and is in `open`; ids in `open` that have no row (call ids, nodes inside a closed
 * one, nodes under another focus) are skipped. Iterative: a 10,000-row tree must not need a deep call stack.
 */
export function flatten(root: TreeNode, open: ReadonlySet<string>, focusId: string | null): FlatRow[] {
  const top = focusId ? findNode(root, focusId) : undefined;
  const first = top ? [top] : root.children;
  const rows: FlatRow[] = [];
  const stack: Item[] = [];
  for (let i = first.length - 1; i >= 0; i--) stack.push({ node: first[i], depth: 0, parent: null, posInSet: i + 1, setSize: first.length, focusRoot: top !== undefined, inClass: false });
  while (stack.length > 0) {
    const item = stack.pop()!;
    const node = item.node;
    const hasChildren = node.children.length > 0;
    const isOpen = hasChildren && (item.focusRoot || open.has(node.id));
    rows.push({ id: node.id, node, depth: item.depth, hasChildren, open: isOpen, parent: item.parent, focusRoot: item.focusRoot, inClass: item.inClass, posInSet: item.posInSet, setSize: item.setSize });
    if (!isOpen) continue;
    const kids = node.children;
    for (let i = kids.length - 1; i >= 0; i--) stack.push({ node: kids[i], depth: item.depth + 1, parent: node.id, posInSet: i + 1, setSize: kids.length, focusRoot: false, inClass: kids[i].kind === "function" && node.kind === "class" });
  }
  return rows;
}
