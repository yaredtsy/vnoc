// Calls as nodes (docs/16-ui-redesign/05-calls-as-nodes.md §1): the id of a call node, how to find it again in a call tree, and the small helpers
// both the page and the host use to read a call tree. Pure: no `vscode`, no React, tested with `node --test`.

import type { CallMark, CallSite, CallTreeNode, CallTreeResult, Range, TreeNode } from "./messages";
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

/** What the session needs to know about a call: the callee (`key` null when the project does not know it), when its tree is known. */
export type CallLookup = (call: CallId) => { key: string | null; name: string } | undefined;

/** The lookup over the trees held. */
export const callLookup =
  (trees: Readonly<Record<string, Pick<CallTreeResult, "root">>>): CallLookup =>
  (id) => {
    const node = resolveCall(trees, id);
    return node ? { key: node.key, name: node.name } : undefined;
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
