// The rows of a card's "Calls" section, made from a call tree. Pure, so it is tested without a browser.

import { markBadge } from "@vnoc/shared/calls.ts";
import type { CallTreeNode } from "@vnoc/shared/messages.ts";

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
