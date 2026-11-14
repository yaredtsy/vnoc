// A simple, deterministic tree layout: one row per visible card, depth to the right. Structure cards and call cards are both rows of it. Pure, so it is
// tested without a browser (docs/16-ui-redesign/03-canvas.md §1, §6; 04-node-cards.md §6).

import { type Calls, callId, callsLook, ownsCalls, stepOf } from "@vnoc/shared/calls.ts";
import type { CallSite, CallTreeNode, Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { type CallLook, callLook } from "../node-card/callLook.ts";
import { type CallsFooter, callsFooter } from "../node-card/callsFooter.ts";
import { SITES_SHOWN } from "../node-card/callMeta.ts";
import type { Contents } from "../node-card/metaOf.ts";

/** A card's width (04 §1): names get about 160 px beside four always-visible buttons. */
export const CARD_W = 300;
/** One column: a card and the room for its edge. */
export const COL_W = CARD_W + 40;
/** The rows of a card (docs/16-ui-redesign/04-node-cards.md §1): header, meta line, a 1 px line, footer. The card's own 1 px border is inside the 80. */
export const HEADER_H = 32;
export const META_H = 24;
export const LINE_H = 1;
export const FOOTER_H = 23;
/** One card: the four rows. */
export const CARD_H = HEADER_H + META_H + LINE_H + FOOTER_H;
/** One card plus a gap. Cards that grow (code, the site list) add their own height, so the layout never has to measure. */
export const ROW_H = CARD_H + 16;
/** A file's code is all of it: the host clamps a range that runs past the end of the document. */
const WHOLE_FILE = { start: { line: 0, character: 0 }, end: { line: 10_000_000, character: 0 } };

/** What an open code view adds to a card. */
export const CODE_H = 220;

/** One line of a call card's open site list (04 §2.1), and the room around the list. */
export const SITE_ROW_H = 18;
export const SITE_LIST_PAD = 8;

/** What an open site list with this many lines adds to a card. */
export const siteListHeight = (rows: number): number => (rows > 0 ? rows * SITE_ROW_H + SITE_LIST_PAD : 0);

/** The height of a drawn card: the base card, its open code view, its open site list. */
export const cardHeight = (p: Pick<Placed, "codeOpen" | "siteRows">): number => CARD_H + (p.codeOpen ? CODE_H : 0) + siteListHeight(p.siteRows);

/** What a call card knows about the call it stands for (04 §2): one callee, called from its parent card, the calls to it merged. */
export type CallInfo = {
  /** how many calls were merged into it (`×N`) */
  count: number;
  /** every site, in the order the core gave them; one with no file has `path: null` */
  sites: CallSite[];
  /** the 1-based line of each site */
  lines: number[];
  /** the parent card's name and file: a site in that file is named by its line alone */
  callerName: string;
  callerPath: string | null;
  look: CallLook;
};

// a `type`, not an interface: React Flow wants node data to be assignable to Record<string, unknown>
export type Placed = {
  /** a structure key, or a call id (`role` says which) */
  id: string;
  role: "node" | "call";
  /** the structure node it shows: itself, or a call's callee; null for a callee the project does not know */
  key: string | null;
  /** the callee's kind for a call card */
  kind: Kind;
  name: string;
  path: string;
  /** 1-based line where the definition starts; null for folders, files and callees with no range */
  line: number | null;
  /** where the definition is in its file; null for folders and files (they have no code to show) */
  range: TreeNode["range"];
  /** the card shows its code (the callee's, for a call) between the meta line and the footer */
  codeOpen: boolean;
  depth: number;
  x: number;
  y: number;
  /** the card this one hangs from (its edge comes from there); null for the first card */
  parentId: string | null;
  /** a function inside a class: its icon is a method's */
  inClass: boolean;
  /** how many structure nodes are directly under it */
  childCount: number;
  /** its calls (a probe, or its tree) say there is something to open: it has a chevron whether or not it has structure children (05 §3.1) */
  callsChevron: boolean;
  /** how many cards a closed card hides: its structure children plus the calls a probe or its tree counted */
  plus: number;
  /** what the footer says about its calls, on the right: nothing, a count, a spinner, a failure with Retry; null: nothing to say (04 §1) */
  callsFooter: CallsFooter;
  /** what a folder or a file holds directly, for its meta line */
  contents: Contents;
  collapsed: boolean;
  syntaxError: boolean;
  /** how many docs are attached, and how the node was found again (see `TreeNode`); a call card shows its callee's */
  docs: number;
  link: TreeNode["link"];
  /** a call card only */
  call: CallInfo | null;
  /** how many lines its open site list has; 0 when closed */
  siteRows: number;
};

/** Whether the card has anything to open: structure under it, or calls. */
export const canOpen = (p: Pick<Placed, "childCount" | "callsChevron">): boolean => p.childCount > 0 || p.callsChevron;

export interface Edge {
  id: string;
  source: string;
  target: string;
  /** `node`: contains (solid); `call`: calls (dashed, 03 §6) */
  kind: "node" | "call";
}

export interface Layout {
  nodes: Placed[];
  edges: Edge[];
  /** structure nodes in the tree that are not drawn because an ancestor is closed */
  hidden: number;
}

const noContents: Contents = { folders: 0, files: 0, classes: 0, functions: 0 };

/** What a node holds directly, by kind. */
function contentsOf(n: TreeNode): Contents {
  const c = { ...noContents };
  for (const child of n.children) {
    if (child.kind === "folder") c.folders++;
    else if (child.kind === "file") c.files++;
    else if (child.kind === "class") c.classes++;
    else c.functions++;
  }
  return c;
}

function size(n: TreeNode): number {
  return 1 + n.children.reduce((sum, c) => sum + size(c), 0);
}

const unknownCalls = (): Calls => ({ state: "unknown" });

/**
 * What is drawn. The root of the view is always open and everything under it starts closed: `expanded` holds only what the user opened (structure
 * keys and call ids), so a new file that appears is closed without any work, and the saved view state stays small.
 *
 * Under an open function (or a class with no `__init__` of its own) come its structure children, then its calls as call cards. A call card's own
 * children are the calls its callee makes. `callsOf` says what is known of each; `sitesOpen`: the call cards whose list of sites is open.
 */
export function layout(
  root: TreeNode,
  expanded: ReadonlySet<string>,
  codeOpen: ReadonlySet<string> = new Set(),
  callsOf: (id: string) => Calls = unknownCalls,
  sitesOpen: ReadonlySet<string> = new Set(),
): Layout {
  const nodes: Placed[] = [];
  const edges: Edge[] = [];
  let y = 0; // an open code view or site list makes a card taller, so rows are stacked by their heights
  const place = (p: Placed) => {
    nodes.push(p);
    y += cardHeight(p) + (ROW_H - CARD_H);
  };

  // every structure node with the kind of its parent: a call card shows its callee's data
  const index = new Map<string, { node: TreeNode; parentKind: Kind | null }>();
  const collect = (n: TreeNode, parentKind: Kind | null) => {
    index.set(n.id, { node: n, parentKind });
    for (const c of n.children) collect(c, n.kind);
  };
  collect(root, null);

  const walk = (n: TreeNode, depth: number, parentId: string | null, parentKind: Kind | null) => {
    // classes and functions have a range; a file's code is all of it; a folder has none
    const range = n.kind === "file" ? WHOLE_FILE : n.range;
    const structural = n.children.length > 0;
    const open = n === root || expanded.has(n.id);
    const calls = ownsCalls(n) ? callsOf(n.id) : null;
    const look = calls ? callsLook(calls, open) : null;
    // a class that has methods is not "calls nothing": its methods are what is under it
    const footer = calls ? callsFooter(calls, open) : null;
    const isCollapsed = (structural || look?.chevron === true) && n !== root && !expanded.has(n.id);
    place({
      id: n.id, role: "node", key: n.id, kind: n.kind, name: n.name, path: n.path, line: n.range ? n.range.start.line + 1 : null, range,
      codeOpen: range !== null && codeOpen.has(n.id), depth, x: depth * COL_W, y, parentId, inClass: n.kind === "function" && parentKind === "class",
      childCount: n.children.length, callsChevron: look?.chevron ?? false, plus: n.children.length + (look?.plus ?? 0), callsFooter: structural && footer?.state === "nothing" ? null : footer, contents: contentsOf(n),
      collapsed: isCollapsed, syntaxError: n.syntax_error, docs: n.docs, link: n.link, call: null, siteRows: 0,
    });
    if (isCollapsed) return;
    for (const c of n.children) {
      edges.push({ id: `${n.id}->${c.id}`, source: n.id, target: c.id, kind: "node" });
      walk(c, depth + 1, n.id, n.kind);
    }
    if (open && calls?.state === "known") drawCalls(n.id, n.id, [], calls.children, depth + 1, { name: n.name, path: n.path });
  };

  /** `owner` and `steps` name the call node in its owner's tree (05 §1); `children`: the calls under `parentId`. */
  const drawCalls = (parentId: string, owner: string, steps: string[], children: readonly CallTreeNode[], depth: number, caller: { name: string; path: string | null }) => {
    for (const c of children) {
      const way = [...steps, stepOf(c, children)];
      const id = callId(owner, way);
      const callee = c.key !== null ? index.get(c.key) : undefined;
      const open = expanded.has(id);
      const calls = callsOf(id);
      const look = callLook({ key: c.key, marks: c.marks, calls }, open);
      const collapsed = look.chevron && !open;
      const range = callee?.node.range ?? null;
      const siteRows = sitesOpen.has(id) && c.call_sites.length > SITES_SHOWN ? c.call_sites.length : 0;
      place({
        id, role: "call", key: c.key, kind: c.kind, name: c.name, path: c.path ?? "", line: range ? range.start.line + 1 : null, range,
        codeOpen: range !== null && codeOpen.has(id), depth, x: depth * COL_W, y, parentId, inClass: c.kind === "function" && callee?.parentKind === "class",
        childCount: 0, callsChevron: look.chevron, plus: look.plus ?? 0, callsFooter: look.unknown || c.marks.includes("recursive") ? null : callsFooter(calls, open), contents: noContents, collapsed, syntaxError: callee?.node.syntax_error ?? false,
        docs: callee?.node.docs ?? 0, link: callee?.node.link ?? null,
        call: { count: c.call_count, sites: c.call_sites, lines: c.call_sites.map((s) => s.start.line + 1), callerName: caller.name, callerPath: caller.path, look },
        siteRows,
      });
      edges.push({ id: `${parentId}->${id}`, source: parentId, target: id, kind: "call" });
      if (collapsed || !look.chevron || calls.state !== "known") continue;
      // a node the walk cut has no calls of its own here: what is under it is its callee's own tree, so the ids below belong to the callee
      const cut = c.children.length === 0 && c.marks.includes("cut") && c.key !== null;
      drawCalls(id, cut ? c.key! : owner, cut ? [] : way, calls.children, depth + 1, { name: c.name, path: c.path });
    }
  };

  walk(root, 0, null, null);
  return { nodes, edges, hidden: size(root) - nodes.filter((p) => p.role === "node").length };
}
