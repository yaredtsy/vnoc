import type { TreeNode } from "./structureView";

// Which node is the cursor in? A lookup in the tree the extension already holds (the tree has every range), not a request to the core.
// Pure and free of the `vscode` module, so it is tested without a VS Code window.

/**
 * The file's node, then the class or function the line is inside, innermost last: `[file, class, method]`.
 * `relativePath` is relative to the opened folder, as in `TreeNode.path`; `line` is 0-based. Empty when the file is not in the tree.
 */
export function chainAt(root: TreeNode, relativePath: string, line: number): TreeNode[] {
  const file = findFile(root, relativePath);
  if (!file) return [];
  const chain = [file];
  for (let node: TreeNode | undefined = file; node; ) {
    node = node.children.find((c) => c.range !== null && c.range.start.line <= line && line <= c.range.end.line);
    if (node) chain.push(node);
  }
  return chain;
}

/** The node whose docs to show: the innermost one that has docs; none has any, the innermost (it shows "Write the first doc"). */
export function docTarget(chain: TreeNode[]): TreeNode | undefined {
  return [...chain].reverse().find((n) => n.docs > 0) ?? chain[chain.length - 1];
}

function findFile(node: TreeNode, relativePath: string): TreeNode | undefined {
  if (node.kind === "file") return node.path === relativePath ? node : undefined;
  for (const child of node.children) {
    const hit = findFile(child, relativePath);
    if (hit) return hit;
  }
  return undefined;
}

/** The node with this id, or undefined. */
export function findNode(root: TreeNode, id: string): TreeNode | undefined {
  if (root.id === id) return root;
  for (const child of root.children) {
    const hit = findNode(child, id);
    if (hit) return hit;
  }
  return undefined;
}

/** The folder or file at this path (relative to the opened folder; `""` is the opened folder itself): what an Explorer right-click is on. */
export function nodeAtPath(root: TreeNode, relativePath: string): TreeNode | undefined {
  if (relativePath === "") return root;
  const search = (node: TreeNode): TreeNode | undefined => {
    if ((node.kind === "folder" || node.kind === "file") && node.path === relativePath) return node;
    for (const child of node.children) {
      const hit = search(child);
      if (hit) return hit;
    }
    return undefined;
  };
  return search(root);
}
