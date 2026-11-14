// What a sidebar row says about its node (docs/16-ui-redesign/07-sidebar-rows.md §1, §6). Pure, so the words and the menu's context are tested.

import type { TreeNode } from "@vnoc/shared/messages.ts";
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

/** What a screen reader says of a row: its kind, its name and how many docs it has. */
export const rowLabel = (node: Pick<TreeNode, "kind" | "name" | "docs">): string => `${node.kind} ${node.name}${node.docs > 0 ? `, ${node.docs} docs` : ""}`;

/** The tooltip of a row that has docs (07 §5, until part 2 draws the hover card): how the docs were found again. */
export const rowTip = (node: Pick<TreeNode, "link">): string | undefined =>
  node.link === "marker" ? "Docs attached: the ID is in the code, so they follow it anywhere" : node.link === "name" ? "Docs attached: found by path and name only, so they are lost if it moves" : undefined;
