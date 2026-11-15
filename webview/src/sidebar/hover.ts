// What the hover card of a row says (docs/16-ui-redesign/07-sidebar-rows.md §5): `checkout · function · app/api.py:11 · 1 doc · ID in the code`. Pure.

import type { TreeNode } from "@vnoc/shared/messages.ts";
import { docsCount } from "../node-card/footerChips.ts";

export interface HoverParts {
  name: string;
  kind: string;
  /** the file, with the line of the definition for a class or function */
  where: string;
  /** "1 doc", "2 docs"; null with none */
  docs: string | null;
  /** how the docs were found again; null with none */
  link: string | null;
}

export function hoverParts(node: Pick<TreeNode, "name" | "kind" | "path" | "range" | "docs" | "link">): HoverParts {
  return {
    name: node.name,
    kind: node.kind,
    where: node.range ? `${node.path}:${node.range.start.line + 1}` : node.path,
    docs: node.docs > 0 ? docsCount(node.docs) : null,
    link: node.docs > 0 ? (node.link === "marker" ? "ID in the code" : node.link === "name" ? "found by name only" : null) : null,
  };
}
