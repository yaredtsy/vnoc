import type { TreeNode } from "./types.ts";

/** The node with this id, or null (it was deleted or renamed since the id was taken). */
export function findNode(root: TreeNode, id: string): TreeNode | null {
  if (root.id === id) return root;
  for (const child of root.children) {
    const hit = findNode(child, id);
    if (hit) return hit;
  }
  return null;
}

/** The nodes from `root` down to the node with this id, or null. The ancestors of a node are all but the last. */
export function pathTo(root: TreeNode, id: string): TreeNode[] | null {
  if (root.id === id) return [root];
  for (const child of root.children) {
    const below = pathTo(child, id);
    if (below) return [root, ...below];
  }
  return null;
}
