// What a sidebar row says about its node (docs/16-ui-redesign/07-sidebar-rows.md §1, §6). Pure, so the words and the menu's context are tested.

import type { TreeNode } from "@vnoc/shared/messages.ts";
import { cardLabel } from "../node-card/labels.ts";
import type { NodeFacts } from "../node-card/nodeContext.ts";
import type { FlatRow } from "./flatten.ts";

/** What the menu reads of a row: the same facts a card gives, from `nodeContext()` (a row has no calls and no code view). */
export const rowFacts = (row: FlatRow): NodeFacts => ({
  id: row.id,
  key: row.id,
  role: "node",
  kind: row.node.kind,
  range: row.node.range,
  codeOpen: false,
  docs: row.node.docs,
  link: row.node.link,
  childCount: row.node.children.length,
  callsChevron: false,
  collapsed: !row.open,
  call: null,
});

/** The dim text after the name: `syntax error`, else `📄 N` when docs are attached, else nothing. */
export const rowDescription = (node: Pick<TreeNode, "syntax_error" | "docs">): string => (node.syntax_error ? "syntax error" : node.docs > 0 ? `📄 ${node.docs}` : "");

/**
 * What a screen reader says of a row (09 §6): `function checkout, selected, 1 doc, cursor here, level 3, 2 of 4`: the card's words (`cardLabel`) and then where it
 * is in the list. The attributes `aria-level`, `aria-posinset` and `aria-setsize` are on the element too, for the tree.
 */
export const rowLabel = (row: Pick<FlatRow, "depth" | "posInSet" | "setSize"> & { node: Pick<TreeNode, "kind" | "name" | "docs" | "syntax_error"> }, state: { selected: boolean; cursor: boolean }): string =>
  `${cardLabel({ role: "node", kind: row.node.kind, name: row.node.name, docs: row.node.docs, lines: [], unknown: false, canOpen: false, collapsed: true, syntaxError: row.node.syntax_error }, state)}, level ${row.depth + 1}, ${row.posInSet} of ${row.setSize}`;
