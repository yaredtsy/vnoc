import type { Placed } from "../canvas/layout.ts";

/**
 * The card's meta line: `path:line` for a definition, the path for a file, `path/` for a folder. The opened folder (id "/") has no path of
 * its own: nothing.
 */
export function whereOf(node: Pick<Placed, "id" | "kind" | "path" | "line">): string {
  if (node.id === "/") return "";
  if (node.line !== null) return `${node.path}:${node.line}`;
  return node.kind === "folder" ? `${node.path}/` : node.path;
}
