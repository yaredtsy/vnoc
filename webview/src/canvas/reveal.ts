// "Show on canvas": make a node visible without changing what the user asked for more than needed. Pure, so it is tested without a browser.

import { pathTo } from "../tree.ts";
import type { TreeNode } from "../types.ts";
import type { FocusStack } from "@vnoc/shared/focusStack.ts";

export interface Reveal {
  /** the focus stack to use: the nearest entry that contains the node, or empty when none does */
  focus: FocusStack;
  /** what to add to the open set so the node is drawn: all its ancestors */
  open: string[];
}

/** `null` when the node is not in the tree (it is gone since the host looked). */
export function reveal(root: TreeNode, focus: FocusStack, id: string): Reveal | null {
  const way = pathTo(root, id);
  if (!way) return null;
  let keep = focus.length;
  // the focus is a history, not a path: walk back to the newest entry whose node contains the one to show
  while (keep > 0 && !contains(root, focus[keep - 1], id)) keep--;
  return { focus: focus.slice(0, keep), open: way.slice(0, -1).map((n) => n.id) };
}

function contains(root: TreeNode, ancestor: string, id: string): boolean {
  return pathTo(root, id)?.some((n) => n.id === ancestor) ?? false;
}
