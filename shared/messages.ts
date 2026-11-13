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
  /** which version of the index this answer is from. Compare two only when their `epoch` is the same: a restarted core counts from the start again */
  revision: number;
  /** which run of the core this answer is from (the host adds it); 1 for the first, one more after each restart */
  epoch?: number;
  git_root: string;
  /** which optional methods the driver has (`callTree`, …); the extension shows a command only when its method is there */
  capabilities: Record<string, boolean>;
  root: TreeNode;
  stats: { folders: number; files: number; classes: number; functions: number };
  truncated: boolean;
  failed: { path: string; error: string }[];
  /** documented nodes that are not in the code now, sorted by path */
  missing: MissingNode[];
  problems: Problem[];
}

/** `vnoc/trash` and `vnoc/restorePlan`: where a node folder moves from and to (absolute paths). The extension does the move. */
export interface MovePlan {
  from: string;
  to: string;
}

/** `vnoc/trashList`: one removed node that can be restored. */
export interface TrashItem {
  /** the entry's folder name in the trash */
  entry: string;
  /** the permanent ID */
  id: string;
  /** UTC, `2026-11-11T10:30:00Z` */
  removed_at: string;
  docs: number;
  /** where the node was: its qualname, or its path for a file or folder */
  name: string;
}

/** `vnoc/trashInfo`: what Empty Trash asks about and deletes. */
export interface TrashInfo {
  /** absolute path of the trash folder */
  folder: string;
  entries: number;
  bytes: number;
}

export interface DocInfo {
  file: string;
  title: string;
  /** absolute path of the Markdown file */
  path: string;
}

export type CallMark = "recursive" | "cut" | "incomplete";

/** One place a node is called: a range, and the file it is in. */
export interface CallSite extends Range {
  /** relative to the opened folder, as `CallTreeNode.path`; null when the file is not under it */
  path: string | null;
}

/** One function or class in a call tree; calls to it from one place are merged into one node (`crates/vnoc/src/protocol.rs` has the Rust side). */
export interface CallTreeNode {
  /** the node's `id` in the structure; null for code the index does not know (a library, an ignored file) */
  key: string | null;
  name: string;
  qualname: string;
  /** relative to the opened folder; null when the code is not under it */
  path: string | null;
  kind: "function" | "class";
  /** how many calls were merged into this node; 1 for the root */
  call_count: number;
  /** where it is called, each with the file it is written in, by file and then in source order; empty for the root */
  call_sites: CallSite[];
  marks: CallMark[];
  children: CallTreeNode[];
}

/** `vnoc/callTree`: the tree of one entry, and the index revision its keys were mapped against. */
export interface CallTreeResult {
  revision: number;
  /** which run of the core made it (the host adds it): see `StructureResult.epoch` */
  epoch?: number;
  root: CallTreeNode;
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
  /** the call trees the host holds, by the key of their entry: a canvas that opens later asks once, then listens for the `callTree` event */
  callTrees: { params: void; result: Record<string, CallTreeResult> };
  /** open a file below the opened folder at a range (a call site) */
  openAt: { params: { path: string; range: Range }; result: void };
  /** the session as it is now; a webview asks once when it starts, and after that it listens for the `session` event */
  session: { params: void; result: Session };
  /** the bar's ⚠: show the NEEDS ATTENTION view (`vnoc.attention.focus`) */
  focusAttention: { params: void; result: void };
  /** Show Log next to Try Again: reveals the "V-NOC" output channel */
  showLog: { params: void; result: void };
}

/** extension → webview. No answer. */
export interface Events {
  indexChanged: { revision: number };
  docText: { path: string; text: string };
  /** the whole session, every time it changes: whole snapshots cannot get out of step, and `seq` drops one that arrives late */
  session: Session;
  /** a menu entry that changes only what the canvas draws (not the session): show a card's code, fit the view */
  viewCommand: { name: "showCode" | "hideCode" | "fitView"; key?: string };
  /** a call tree arrived: the entry's card shows it in its "Calls" section */
  callTree: { key: string; result: CallTreeResult };
  /** what the host is doing about the call tree of this entry: the canvas draws a spinner for `loading` and a retry for `failed`; `idle` is "nothing in flight" */
  callTreeState: { key: string; state: "loading" | "failed" | "idle"; message?: string };
  /** the core was restarted: every call tree the host held is gone, and so must be the page's (revisions before and after cannot be compared) */
  callTreesReset: Record<string, never>;
  /** the docs tab beside an editor is already open: show this node and stay on it (pinned) */
  showNode: { id: string };
}

/** webview → extension, no answer. */
export type Notes =
  | { type: "intent"; context: ContextId; intent: Intent } // the one way a webview changes the session
  | { type: "rendered"; visible: number; hidden: number; calls: number } // the extension's test suite uses it; `calls`: how many cards draw a "Calls" section
  | { type: "warned"; level: "none" | "warning" | "error"; text: string } // the bar's ⚠ changed: the extension's test suite uses it
  | { type: "close" } // the docs tab
  | { type: "error"; message: string }; // a page crashed while drawing: the host logs it, and the test suite fails on it

// What actually travels. `postMessage` carries one of these:
export type RequestMessage = { [M in keyof Requests]: { type: "req"; id: number; method: M; params: Requests[M]["params"] } }[keyof Requests];
export type ResponseMessage = { type: "res"; id: number; result: unknown } | { type: "res"; id: number; error: string };
export type EventMessage = { [E in keyof Events]: { type: "event"; name: E; data: Events[E] } }[keyof Events];
