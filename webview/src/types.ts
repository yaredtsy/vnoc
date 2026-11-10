// The shape the core sends for `vnoc/structure` (crates/vnoc/src/protocol.rs and structure.rs). Keep in sync until the generated types arrive.

export type Kind = "folder" | "file" | "class" | "function";

export interface TreeNode {
  /** unique in the tree: the path for folders and files, the location key (`path::qualname#n`) for defs */
  id: string;
  kind: Kind;
  name: string;
  path: string;
  qualname: string | null;
  range: { start: { line: number; character: number }; end: { line: number; character: number } } | null;
  syntax_error: boolean;
  children: TreeNode[];
}

export interface StructureResult {
  root: TreeNode;
  stats: { folders: number; files: number; classes: number; functions: number; ms: number };
  truncated: boolean;
  failed: { path: string; error: string }[];
}
