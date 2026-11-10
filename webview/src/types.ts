// The shape the core sends for `vnoc/structure` (crates/vnoc/src/protocol.rs and index/tree.rs). Keep in sync until the generated types arrive.

export type Kind = "folder" | "file" | "class" | "function";

export interface TreeNode {
  /** unique in the tree: the repo path (from the git root) for folders and files, the location key (`path::qualname#n`) for defs */
  id: string;
  kind: Kind;
  name: string;
  /** relative to the opened folder; empty for the root */
  path: string;
  qualname: string | null;
  range: { start: { line: number; character: number }; end: { line: number; character: number } } | null;
  syntax_error: boolean;
  /** the permanent ID, when someone attached a doc to this node */
  node_id: string | null;
  /** how many docs are attached */
  docs: number;
  /** how it was found again: the ID is in the code ("marker"), or only the stored path and name matched ("name") */
  link: "marker" | "name" | null;
  children: TreeNode[];
}

export interface StructureResult {
  /** which version of the index this answer is from; a lower number than the one on screen is a late answer */
  revision: number;
  git_root: string;
  root: TreeNode;
  stats: { folders: number; files: number; classes: number; functions: number };
  truncated: boolean;
  failed: { path: string; error: string }[];
}
