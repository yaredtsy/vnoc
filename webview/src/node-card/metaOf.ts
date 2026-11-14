// The card's meta line: where it is, said only as far as the card it hangs from does not already say (docs/16-ui-redesign/04-node-cards.md §1).
// Pure, so the table is tested; NodeMeta only draws it. A call card has its own meta line (`callMeta`).

import type { Placed } from "../canvas/layout.ts";

/** What a folder or a file holds directly: a folder its folders and files, a file its top-level classes and functions. */
export type Contents = { folders: number; files: number; classes: number; functions: number };

export interface Meta {
  text: string;
  /** the full path (`app/core/models/conversation_state.py:382`), always the line's tooltip */
  tip: string;
  /** drawn in the error colour: the file did not parse */
  error: boolean;
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
const fileName = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** `2 classes · 9 functions`, `3 folders · 12 files`; the parts that are 0 are left out; `empty` when none is left. */
export function contentsText(kind: "folder" | "file", c: Contents): string {
  const parts = kind === "folder" ? [c.folders > 0 && plural(c.folders, "folder", "folders"), c.files > 0 && plural(c.files, "file", "files")] : [c.classes > 0 && plural(c.classes, "class", "classes"), c.functions > 0 && plural(c.functions, "function", "functions")];
  const text = parts.filter(Boolean).join(" · ");
  return text === "" ? "empty" : text;
}

/**
 * The meta line of a structure card, relative to its parent card (`parentId`: null when it is the root of the view):
 * a definition under its file or class says `line 382`, as the root `conversation_state.py:382`; a file or folder under its folder says what is inside;
 * as the root, its path (the box cuts it at the start). A file that did not parse says `syntax error` instead. The opened folder has no path: nothing.
 */
export function metaOf(node: Pick<Placed, "id" | "kind" | "path" | "line" | "parentId" | "syntaxError" | "contents">): Meta {
  const tip = node.id === "/" ? "" : node.line !== null ? `${node.path}:${node.line}` : node.kind === "folder" ? `${node.path}/` : node.path;
  if (node.syntaxError) return { text: "syntax error", tip, error: true };
  const rooted = node.parentId === null;
  if (node.kind === "folder" || node.kind === "file") {
    if (rooted) return { text: tip, tip, error: false };
    return { text: contentsText(node.kind, node.contents), tip, error: false };
  }
  if (node.line === null) return { text: rooted ? fileName(node.path) : "", tip, error: false };
  return { text: rooted ? `${fileName(node.path)}:${node.line}` : `line ${node.line}`, tip, error: false };
}
