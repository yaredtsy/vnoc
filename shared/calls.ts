// Calls as nodes (docs/16-ui-redesign/05-calls-as-nodes.md §1): the id of a call node, how to find it again in a call tree, and the small helpers
// both the page and the host use to read a call tree. Pure: no `vscode`, no React, tested with `node --test`.

import type { CallMark, CallSite, CallTreeNode, CallTreeResult, Range, TreeNode } from "./messages";
import type { Session } from "./session.ts";
import { findNode } from "./tree.ts";

/** `call:` + the JSON of `[owner, step, step, …]`: the path of keys from the function that owns the tree to this node. */
export type CallId = string;

const PREFIX = "call:";

/** `call:["app/api.py::checkout","app/order.py::checkout_order","app/inventory.py::reserve_stock"]`. JSON, because a key may hold any character a path can. */
export function callId(owner: string, steps: readonly string[]): CallId {
  return PREFIX + JSON.stringify([owner, ...steps]);
}

export const isCallId = (id: string): id is CallId => id.startsWith(PREFIX);

/** The owner and the steps of a call id; null for anything that is not one. */
export function parseCallId(id: string): { owner: string; steps: string[] } | null {
  if (!isCallId(id)) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(id.slice(PREFIX.length));
  } catch {
    return null;
  }
  if (!Array.isArray(parsed) || parsed.length < 1 || !parsed.every((x) => typeof x === "string")) return null;
  const [owner, ...steps] = parsed as string[];
  return { owner, steps };
}

/**
 * One step of a call id: the callee's key; for a callee the index does not know, `?` + its path + `::` + its qualname. A second sibling with the
 * same step gets `#2`, a third `#3`: two calls to one callee are one node in a merged tree, but twins can still come (an unknown callee in two files).
 * Keys, not child indexes: a call written above an old one moves every index, and with them what is open and selected; keys stay.
 */
export function stepOf(node: CallTreeNode, siblings: readonly CallTreeNode[]): string {
  const base = (n: CallTreeNode) => n.key ?? `?${n.path ?? ""}::${n.qualname}`;
  const mine = base(node);
  let before = 0;
  for (const sibling of siblings) {
    if (sibling === node) break;
    if (base(sibling) === mine) before++;
  }
  return before === 0 ? mine : `${mine}#${before + 1}`;
}

/** Every node under the owner's entry with its id, parents before children. The entry itself is the owner, not a call node. */
export function callNodesIn(owner: string, root: CallTreeNode): { id: CallId; node: CallTreeNode }[] {
  const out: { id: CallId; node: CallTreeNode }[] = [];
  const walk = (parent: CallTreeNode, steps: string[]) => {
    for (const node of parent.children) {
      const way = [...steps, stepOf(node, parent.children)];
      out.push({ id: callId(owner, way), node });
      walk(node, way);
    }
  };
  walk(root, []);
  return out;
}

/** The call node an id names, in the trees we hold (by owner key); undefined when the owner's tree is not here or the path is not in it. */
export function resolveCall(trees: Readonly<Record<string, Pick<CallTreeResult, "root">>>, id: CallId): CallTreeNode | undefined {
  const parsed = parseCallId(id);
  const tree = parsed && trees[parsed.owner];
  if (!parsed || !tree) return undefined;
  let node: CallTreeNode = tree.root;
  for (const step of parsed.steps) {
    const next = node.children.find((child) => stepOf(child, node.children) === step);
    if (!next) return undefined;
    node = next;
  }
  return parsed.steps.length === 0 ? undefined : node;
}

/**
 * What the session needs to know about a call: the callee (`key` null when the project does not know it).
 * `undefined`: the owner's tree is not held, so nothing is known (not loaded yet, failed, dropped): "unknown" is not "gone".
 * `null`: the owner's tree is held and the call is not in it: gone.
 */
export type CallLookup = (call: CallId) => { key: string | null; name: string } | null | undefined;

/** The lookup over the trees held. */
export const callLookup =
  (trees: Readonly<Record<string, Pick<CallTreeResult, "root">>>): CallLookup =>
  (id) => {
    const parsed = parseCallId(id);
    if (!parsed || !trees[parsed.owner]) return undefined;
    const node = resolveCall(trees, id);
    return node ? { key: node.key, name: node.name } : null;
  };

/**
 * Whether the keys inside a call id still exist in the structure (no call tree needed): the owner and every step that names a node. A callee the
 * index does not know (`?…`) cannot be checked and counts as there. False for anything that is not a call id.
 */
export function callExists(tree: TreeNode, id: string): boolean {
  const parsed = parseCallId(id);
  if (!parsed) return false;
  const known = (key: string) => findNode(tree, key) !== undefined;
  return known(parsed.owner) && parsed.steps.every((step) => step.startsWith("?") || known(step) || (/#\d+$/.test(step) && known(step.replace(/#\d+$/, ""))));
}

// ── moved here from webview/src/canvas/calls.ts: both sides read call trees ─────────────────────────────────────────

const MARKS: Record<CallMark, { text: string; title: string }> = {
  recursive: { text: "↻ recursive", title: "It is already above on this path: shown, not entered" },
  cut: { text: "✂ cut", title: "Its calls are not shown: the tree stopped here" },
  incomplete: { text: "… incomplete", title: "The analysis gave up on part of this" },
};

export const markBadge = (mark: CallMark) => MARKS[mark];

/** "line 12": a call site as the menu names it (0-based in the message, 1-based for people). */
export const siteLabel = (site: Range): string => `line ${site.start.line + 1}`;

/** A site the card can open: it names a file. */
export type OpenableSite = CallSite & { path: string };

/** The sites of a node that can be opened. One with no file (the driver gave none and the parent is outside the folder) has nowhere to go. */
export const listedSites = (node: CallTreeNode): OpenableSite[] => node.call_sites.filter((site): site is OpenableSite => site.path !== null);

/** A site as the list names it: `line 12` in the parent's file, `base.py:12` in another one, so a jump to another file is not a surprise. */
export const siteText = (site: OpenableSite, parentPath: string | null): string =>
  site.path === parentPath ? siteLabel(site) : `${site.path.slice(site.path.lastIndexOf("/") + 1)}:${site.start.line + 1}`;

/** The code changed since this tree was made: the index is on a newer revision than the one its keys were mapped against. */
export const isStale = (tree: CallTreeResult, indexRevision: number): boolean => tree.revision < indexRevision;

/**
 * `isStale` against a structure, and never across a core restart: a restarted core counts revisions from the start again, so a revision from before
 * and one from after say nothing about which is newer. Different runs of the core are not stale; the host drops the old trees, and the page
 * learns it from `callTreesReset`.
 */
export const isStaleIn = (tree: CallTreeResult, structure: { revision: number; epoch?: number }): boolean =>
  (tree.epoch === undefined || structure.epoch === undefined || tree.epoch === structure.epoch) && isStale(tree, structure.revision);

// ── what is under a function or a call node (05 §4) ─────────────────────────────────────────────────────────────────

/** What the host holds about call trees: the trees, and what it is doing about each entry. */
export interface TreeStore {
  trees: Readonly<Record<string, Pick<CallTreeResult, "root">>>;
  states?: Readonly<Record<string, { state: "loading" | "failed" | "idle"; message?: string }>>;
}

export type Calls =
  | { state: "unknown" } // the owner's tree is not here: draw a chevron, no +N
  | { state: "loading" }
  | { state: "failed"; message: string }
  | { state: "known"; children: CallTreeNode[] };

/**
 * The calls under a structure function (its key) or a call node (its id), as the canvas draws them one level at a time.
 * A call node that is `cut` (the walk stopped there) and has no children shows the root children of its callee's own tree instead.
 */
export function callsOf(store: TreeStore, id: string): Calls {
  const own = (key: string): Calls => {
    const tree = store.trees[key];
    if (tree) return { state: "known", children: tree.root.children };
    const state = store.states?.[key];
    if (state?.state === "loading") return { state: "loading" };
    if (state?.state === "failed") return { state: "failed", message: state.message ?? "" };
    return { state: "unknown" };
  };
  if (!isCallId(id)) return own(id);
  const node = resolveCall(store.trees, id);
  if (!node) {
    // the owner's tree is held but the call is not in it: it is gone, nothing is under it. Not held: what the owner's state says
    const owner = parseCallId(id)?.owner ?? id;
    return store.trees[owner] ? { state: "known", children: [] } : own(owner);
  }
  if (node.children.length > 0 || !node.marks.includes("cut") || node.key === null) return { state: "known", children: node.children };
  return own(node.key);
}

// ── what the host should have (05 §2) ───────────────────────────────────────────────────────────────────────────────

export interface Wanted {
  key: string;
  /** `open`: an open card or the last focus step; `cut`: the callee of an open call node the walk stopped at; `prefetch`: the selected one, so a first click is instant */
  reason: "open" | "cut" | "prefetch";
}

const isEntry = (n: TreeNode | undefined): n is TreeNode => n !== undefined && (n.kind === "function" || n.kind === "class");

/**
 * The call trees to have, in the order to fetch them: first what is drawn open (in every context, in the order of `session.order`), then the
 * callees of open call nodes that were cut, then the selected function of the active context. Each key once; a closed card is no longer in it.
 * Only a function or a class owns a tree.
 */
export function wantedCalls(session: Session, tree: TreeNode, trees: TreeStore["trees"]): Wanted[] {
  const out: Wanted[] = [];
  const seen = new Set<string>();
  const add = (key: string | undefined, reason: Wanted["reason"]) => {
    if (key === undefined || seen.has(key) || !isEntry(findNode(tree, key))) return;
    seen.add(key);
    out.push({ key, reason });
  };
  const cut: string[] = [];
  for (const id of session.order) {
    const c = session.contexts[id];
    for (const open of c.expanded) {
      if (!isCallId(open)) add(open, "open");
      else {
        const owner = parseCallId(open)?.owner;
        add(owner, "open");
        const node = resolveCall(trees, open);
        if (node && node.key !== null && node.children.length === 0 && node.marks.includes("cut")) cut.push(node.key);
      }
    }
    add(c.focus[c.focus.length - 1], "open");
  }
  cut.forEach((key) => add(key, "cut"));
  const selected = session.contexts[session.active]?.selected;
  if (selected !== null && selected !== undefined && !isCallId(selected)) add(selected, "prefetch");
  return out;
}
