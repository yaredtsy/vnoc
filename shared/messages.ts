// Everything that crosses between the extension host and a webview, declared once. A message that is not in this file does not exist
// (docs/10-ui-rework/01-state.md §4). Types only: nothing here runs, so any side can import it.

import type { ContextId, Intent, Session } from "./session";

export type Kind = "folder" | "file" | "class" | "function";

export interface Position {
  line: number;
  character: number;
}

export interface Range {
  start: Position;
  end: Position;
}

/** A node as the UI sees it (`crates/vnoc/src/index/tree.rs` is the Rust side of the same shape). */
export interface TreeNode {
  /** the location key: the repo path for a folder or file, `path::qualname` (or `#n`) for a class or function. Changes on rename. */
  id: string;
  kind: Kind;
  name: string;
  /** relative to the opened folder (not the git root); empty for the root */
  path: string;
  qualname: string | null;
  range: Range | null;
  syntax_error: boolean;
  /** the permanent 26-character ID, once a doc is attached */
  node_id: string | null;
  /** how many docs are attached */
  docs: number;
  /** how the node was found again: the ID is in the code ("marker"), or only the stored path and name matched ("name") */
  link: "marker" | "name" | null;
  children: TreeNode[];
}

/** A documented node nothing in the code matches any more (Needs Attention). The anchor is as `node.toml` says it. */
export interface MissingNode {
  /** the permanent ID */
  id: string;
  /** "function" | "class" | "file" | "folder", a value a newer V-NOC wrote, or null when `node.toml` does not say */
  kind: string | null;
  path: string;
  qualname: string;
  docs: number;
}

/** Something to look at: where to go when it is clicked. */
export interface Problem {
  text: string;
  /** absolute path */
  file: string | null;
  /** 0-based */
  line: number | null;
}

export interface StructureResult {
  /** which version of the index this answer is from; a lower number than the one shown is a late answer */
  revision: number;
  git_root: string;
  root: TreeNode;
  stats: { folders: number; files: number; classes: number; functions: number };
  truncated: boolean;
  failed: { path: string; error: string }[];
  /** documented nodes that are not in the code now, sorted by path */
  missing: MissingNode[];
  problems: Problem[];
}

export interface DocInfo {
  file: string;
  title: string;
  /** absolute path of the Markdown file */
  path: string;
}

/** webview → extension, with an answer. One line per request: its parameters and its result. */
export interface Requests {
  structure: { params: void; result: StructureResult };
  /** the permanent ID (`TreeNode.node_id`), not the location key; the relay renames it for the core */
  docs: { params: { nodeId: string }; result: DocInfo[] };
  docText: { params: { path: string }; result: string };
  code: { params: { path: string; range: Range }; result: string };
  addDoc: { params: { key: string }; result: { added: boolean } };
  openDoc: { params: { path: string }; result: void };
  openLink: { params: { href: string; from: string }; result: void };
  goToCode: { params: { key: string }; result: void };
  /** the session as it is now; a webview asks once when it starts, and after that it listens for the `session` event */
  session: { params: void; result: Session };
}

/** extension → webview. No answer. */
export interface Events {
  indexChanged: { revision: number };
  docText: { path: string; text: string };
  /** the whole session, every time it changes: whole snapshots cannot get out of step, and `seq` drops one that arrives late */
  session: Session;
  /** a menu entry that changes only what the canvas draws (not the session): show a card's code, fit the view */
  viewCommand: { name: "showCode" | "hideCode" | "fitView"; key?: string };
  /** the docs tab beside an editor is already open: show this node and stay on it (pinned) */
  showNode: { id: string };
}

/** webview → extension, no answer. */
export type Notes =
  | { type: "intent"; context: ContextId; intent: Intent } // the one way a webview changes the session
  | { type: "rendered"; visible: number; hidden: number } // the extension's test suite uses it
  | { type: "close" } // the docs tab
  | { type: "error"; message: string }; // a page crashed while drawing: the host logs it, and the test suite fails on it

// What actually travels. `postMessage` carries one of these:
export type RequestMessage = { [M in keyof Requests]: { type: "req"; id: number; method: M; params: Requests[M]["params"] } }[keyof Requests];
export type ResponseMessage = { type: "res"; id: number; result: unknown } | { type: "res"; id: number; error: string };
export type EventMessage = { [E in keyof Events]: { type: "event"; name: E; data: Events[E] } }[keyof Events];
