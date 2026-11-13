import { useMemo } from "react";

import type { TreeNode } from "@vnoc/shared/messages.ts";
import { findNode } from "@vnoc/shared/tree.ts";

export interface Crumb {
  id: string;
  node: TreeNode | null;
}

/** The focus path as nodes: one crumb per step, root first. The node is null for a step the tree no longer has. */
export function useCrumbs(root: TreeNode | null, focus: readonly string[]): Crumb[] {
  return useMemo(() => (root ? focus.map((id) => ({ id, node: findNode(root, id) ?? null })) : []), [root, focus]);
}
