// A simple, deterministic tree layout: one row per visible node, depth to the right. Pure, so it is tested without a browser.

import { type Calls, callsLook } from "@vnoc/shared/calls.ts";
import type { Kind, TreeNode } from "@vnoc/shared/messages.ts";

export const COL_W = 300;
/** One card: header 30 + meta 22 + footer 20 (docs/16-ui-redesign/04-node-cards.md §1). */
export const CARD_H = 72;
/** One card plus a gap. Cards that grow (code, calls) add their own height, so the layout never has to measure. */
export const ROW_H = CARD_H + 16;
/** A file's code is all of it: the host clamps a range that runs past the end of the document. */
const WHOLE_FILE = { start: { line: 0, character: 0 }, end: { line: 10_000_000, character: 0 } };

/** What an open code view adds to a card. */
export const CODE_H = 220;

/** A card's "Calls" section: a header line, then the rows, up to a limit (more scroll inside it). A tree with no rows still has one line, "calls nothing". */
export const CALLS_HEAD_H = 24;
export const CALL_ROW_H = 20;
export const CALLS_MAX_ROWS = 10;

/** What a "Calls" section with this many rows adds to a card; nothing for `null` (the card has no section). */
export const callsHeight = (rows: number | null): number => (rows === null ? 0 : CALLS_HEAD_H + Math.min(Math.max(rows, 1), CALLS_MAX_ROWS) * CALL_ROW_H);

/** The height of a drawn card: the base card, its open code view, its "Calls" section. */
export const cardHeight = (p: Pick<Placed, "codeOpen" | "callRows">): number => CARD_H + (p.codeOpen ? CODE_H : 0) + callsHeight(p.callRows);

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
  /** how many structure nodes are directly under it */
  childCount: number;
  /** a function or class with no structure children whose calls (a probe, or its tree) say there is something to open: it has a chevron too (05 §3.1) */
  callsChevron: boolean;
  /** how many nodes a closed card hides: its structure children, or the calls a probe or its tree counted */
  plus: number;
  /** its calls are known and there are none: the meta line says so */
  callsNothing: boolean;
  collapsed: boolean;
  syntaxError: boolean;
  /** how many docs are attached, and how the node was found again (see `TreeNode`) */
  docs: number;
  link: TreeNode["link"];
  /** how many rows the card's "Calls" section has; `null` when the card has no such section (0 rows: it calls nothing) */
  callRows: number | null;
};

/** Whether the card has anything to open: structure under it, or calls. */
export const canOpen = (p: Pick<Placed, "childCount" | "callsChevron">): boolean => p.childCount > 0 || p.callsChevron;

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
export function layout(
  root: TreeNode,
  expanded: ReadonlySet<string>,
  codeOpen: ReadonlySet<string> = new Set(),
  callRows: ReadonlyMap<string, number> = new Map(),
  /** what is known about the calls of a function or class: a probe, its tree, or nothing yet */
  callsOf: (key: string) => Calls = () => ({ state: "unknown" }),
): Layout {
  const nodes: Placed[] = [];
  const edges: Edge[] = [];
  let y = 0; // an open code view makes a card taller, so rows are stacked by their heights
  const walk = (n: TreeNode, depth: number) => {
    // classes and functions have a range; a file's code is the whole file; a folder has none
    const range = n.kind === "file" ? WHOLE_FILE : n.range;
    const structural = n.children.length > 0;
    // a function or class with nothing under it in the structure may still have calls to open
    const look = !structural && (n.kind === "function" || n.kind === "class") ? callsLook(callsOf(n.id), n === root || expanded.has(n.id)) : null;
    const isCollapsed = (structural || look?.chevron === true) && n !== root && !expanded.has(n.id);
    nodes.push({ id: n.id, kind: n.kind, name: n.name, path: n.path, line: n.range ? n.range.start.line + 1 : null, range, codeOpen: range !== null && codeOpen.has(n.id), depth, x: depth * COL_W, y, childCount: n.children.length, callsChevron: look?.chevron ?? false, plus: structural ? n.children.length : look?.plus ?? 0, callsNothing: look?.callsNothing ?? false, collapsed: isCollapsed, syntaxError: n.syntax_error, docs: n.docs, link: n.link, callRows: callRows.get(n.id) ?? null });
    y += ROW_H + (range !== null && codeOpen.has(n.id) ? CODE_H : 0) + callsHeight(callRows.get(n.id) ?? null);
    if (isCollapsed) return;
    for (const c of n.children) {
      edges.push({ id: `${n.id}->${c.id}`, source: n.id, target: c.id });
      walk(c, depth + 1);
    }
  };
  walk(root, 0);
  return { nodes, edges, hidden: size(root) - nodes.length };
}
