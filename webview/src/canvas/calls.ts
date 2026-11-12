// The rows of a card's "Calls" section, made from a call tree. Pure, so it is tested without a browser.

import type { CallMark, CallSite, CallTreeNode, CallTreeResult, Range } from "@vnoc/shared/messages.ts";

export interface CallRow {
  /** unique inside one tree: the child indexes from the entry down, `0.2.1` */
  id: string;
  /** 0 for what the entry calls */
  depth: number;
  node: CallTreeNode;
  /** `save`, or `save ×2` when calls were merged */
  label: string;
  marks: { text: string; title: string }[];
  /** the parent node's file: a site in it is labelled by its line alone; null when the parent is not a file below the opened folder */
  parentPath: string | null;
}

const MARKS: Record<CallMark, { text: string; title: string }> = {
  recursive: { text: "↻ recursive", title: "It is already above on this path: shown, not entered" },
  cut: { text: "✂ cut", title: "Its calls are not shown: the tree stopped here" },
  incomplete: { text: "… incomplete", title: "The analysis gave up on part of this" },
};

export const markBadge = (mark: CallMark) => MARKS[mark];

export const callLabel = (node: CallTreeNode): string => (node.call_count > 1 ? `${node.name} ×${node.call_count}` : node.name);

/** One row for each node under the entry, parents before children, in the tree's order. The entry itself is the card, not a row. */
export function callRows(root: CallTreeNode): CallRow[] {
  const rows: CallRow[] = [];
  const walk = (parent: CallTreeNode, depth: number, prefix: string) => {
    parent.children.forEach((node, i) => {
      const id = prefix === "" ? `${i}` : `${prefix}.${i}`;
      rows.push({ id, depth, node, label: callLabel(node), marks: node.marks.map(markBadge), parentPath: parent.path });
      walk(node, depth + 1, id);
    });
  };
  walk(root, 0, "");
  return rows;
}

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
