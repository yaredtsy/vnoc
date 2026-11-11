// Looking things up in the tree. Pure, so the extension and the webview use the same code and it is tested without either.

import type { MissingNode, TreeNode } from "./messages";

/** The node with this id, or undefined. */
export function findNode(root: TreeNode, id: string): TreeNode | undefined {
  if (root.id === id) return root;
  for (const child of root.children) {
    const hit = findNode(child, id);
    if (hit) return hit;
  }
  return undefined;
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

/** Every node under and including `root`, parents before children (what a search lists). */
export function allNodes(root: TreeNode): TreeNode[] {
  return [root, ...root.children.flatMap(allNodes)];
}

/**
 * The file's node, then the class or function the line is inside, innermost last: `[file, class, method]`.
 * `relativePath` is relative to the opened folder, as in `TreeNode.path`; `line` is 0-based. Empty when the file is not in the tree.
 */
export function chainAt(root: TreeNode, relativePath: string, line: number): TreeNode[] {
  const file = allNodes(root).find((n) => n.kind === "file" && n.path === relativePath);
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

/** The folder or file at this path (relative to the opened folder; `""` is the opened folder itself): what an Explorer right-click is on. */
export function nodeAtPath(root: TreeNode, relativePath: string): TreeNode | undefined {
  if (relativePath === "") return root;
  return allNodes(root).find((n) => (n.kind === "folder" || n.kind === "file") && n.path === relativePath);
}

/** The docs tab's pin for a Missing node: it is not in the tree, so it has no location key. The permanent ID follows the prefix. */
export const MISSING_PIN = "missing:";

/** A Missing node drawn as a node, so the doc panel can show its docs: titled with the stored name, shown at the stored place. */
export function missingAsNode(m: MissingNode): TreeNode {
  const kind = m.kind === "class" || m.kind === "folder" || m.kind === "file" ? m.kind : m.qualname ? "function" : "file";
  return { id: MISSING_PIN + m.id, kind, name: m.qualname || m.path, path: m.path, qualname: m.qualname || null, range: null, syntax_error: false, node_id: m.id, docs: m.docs, link: null, children: [] };
}
