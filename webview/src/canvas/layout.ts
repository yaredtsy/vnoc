// A simple, deterministic tree layout: one row per visible node, depth to the right. Pure, so it is tested without a browser.

import type { Kind, TreeNode } from "../types.ts";

export const COL_W = 300;
/** One card (80 px, see node-card.css) plus a gap. Fixed until the code toggle makes cards grow and the layout has to measure them. */
export const ROW_H = 96;
/** A file's code is all of it: the host clamps a range that runs past the end of the document. */
const WHOLE_FILE = { start: { line: 0, character: 0 }, end: { line: 10_000_000, character: 0 } };

/** What an open code view adds to a card (80 + 220 = the 300 px of `.node-card.code-open`). */
export const CODE_H = 220;

// a `type`, not an interface: React Flow wants node data to be assignable to Record<string, unknown>
export type Placed = {
  id: string;
  kind: Kind;
  name: string;
  path: string;
  /** 1-based line where the definition starts; null for folders and files */
  line: number | null;
  /** where the definition is in its file; null for folders and files (they have no code to show) */
  range: TreeNode["range"];
  /** the card shows its code instead of the body */
  codeOpen: boolean;
  depth: number;
  x: number;
  y: number;
  /** how many nodes are directly under it (shown on a collapsed node) */
  childCount: number;
  collapsed: boolean;
  syntaxError: boolean;
  /** how many docs are attached, and how the node was found again (see `TreeNode`) */
  docs: number;
  link: TreeNode["link"];
};

export interface Edge {
  id: string;
  source: string;
  target: string;
}

export interface Layout {
  nodes: Placed[];
  edges: Edge[];
  /** nodes in the tree that are not drawn because an ancestor is collapsed */
  hidden: number;
}

function size(n: TreeNode): number {
  return 1 + n.children.reduce((sum, c) => sum + size(c), 0);
}

/**
 * What is drawn. The root of the view is always open and everything under it starts closed: `expanded` holds only what the user opened,
 * so a new file that appears is closed without any work, and the saved view state stays small.
 */
export function layout(root: TreeNode, expanded: ReadonlySet<string>, codeOpen: ReadonlySet<string> = new Set()): Layout {
  const nodes: Placed[] = [];
  const edges: Edge[] = [];
  let y = 0; // an open code view makes a card taller, so rows are stacked by their heights
  const walk = (n: TreeNode, depth: number) => {
    // classes and functions have a range; a file's code is the whole file; a folder has none
    const range = n.kind === "file" ? WHOLE_FILE : n.range;
    const isCollapsed = n.children.length > 0 && n !== root && !expanded.has(n.id);
    nodes.push({ id: n.id, kind: n.kind, name: n.name, path: n.path, line: n.range ? n.range.start.line + 1 : null, range, codeOpen: range !== null && codeOpen.has(n.id), depth, x: depth * COL_W, y, childCount: n.children.length, collapsed: isCollapsed, syntaxError: n.syntax_error, docs: n.docs, link: n.link });
    y += ROW_H + (range !== null && codeOpen.has(n.id) ? CODE_H : 0);
    if (isCollapsed) return;
    for (const c of n.children) {
      edges.push({ id: `${n.id}->${c.id}`, source: n.id, target: c.id });
      walk(c, depth + 1);
    }
  };
  walk(root, 0);
  return { nodes, edges, hidden: size(root) - nodes.length };
}
