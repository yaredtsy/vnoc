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

/** `likely`: the engine's best guess for the callee, not a certainty (the Python driver sends it from analyzer D28) */
export type CallMark = "recursive" | "cut" | "incomplete" | "likely";

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

/** `eol`: the document's line ending (`document.eol`), which `reindent` joins the written lines with. */
export type EditStartResult = { dirty: true } | { text: string; version: number; prefix: string; eol: "\n" | "\r\n" };

/** What a draft sends to be written: where it was, what it started from (`version`, `base`), how to put the indent and the line endings back, and the text. */
export interface EditSaveParams {
  /** the draft's key: the structure node, which the core looks for when the document moved on */
  key: string;
  path: string;
  range: Range;
  version: number;
  base: string;
  prefix: string;
  eol: "\n" | "\r\n";
  text: string;
}
export type EditSaveResult = { saved: true } | { written: true } | { conflict: true } | { gone: true } | { cancelled: true };

/** What "Show Diff" needs: the draft, and what it started from, so the host can put the draft where the function is now. */
export interface EditDiffParams {
  key: string;
  path: string;
  range: Range;
  base: string;
  prefix: string;
  eol: "\n" | "\r\n";
  text: string;
}

/** webview → extension, with an answer. One line per request: its parameters and its result. */
export interface Requests {
  structure: { params: void; result: StructureResult };
  /** the permanent ID (`TreeNode.node_id`), not the location key; the relay renames it for the core */
  docs: { params: { nodeId: string }; result: DocInfo[] };
  docText: { params: { path: string }; result: string };
  code: { params: { path: string; range: Range }; result: string };
  /**
   * Edit on a card (docs/17-edit-in-canvas.md §5): the host opens the document. Dirty (and no `saveFirst`): `{ dirty: true }`, nothing else. Otherwise the text of
   * the range, the document's `version`, and the `prefix`: the first line's text before `range.start`. `saveFirst` saves a dirty document first ("Save and Edit"):
   * the range is the one the card sent, which is stale until the index has read the save, so the card does not start a draft from that answer.
   */
  editStart: { params: { path: string; range: Range; saveFirst?: boolean }; result: EditStartResult };
  /**
   * Save on a draft (docs/17-edit-in-canvas.md §6): the host checks the document's version and the text at `range` against the draft's `version` and `base`, and
   * writes the draft re-indented into the range. `saved`: written and the file saved. `written`: written into the buffer, not saved (the file was dirty, E4).
   * `conflict`: nothing was written (the document moved on, or the function changed). `gone`: nothing was written (the text has no such function). A draft that was
   * saved already (a second Save) answers `saved` and writes nothing. `cancelled`: the draft takes the function's `ID:` away, the host asked, and the answer was Cancel:
   * nothing was written, and the draft stays as it was (§7).
   */
  editSave: { params: EditSaveParams; result: EditSaveResult };
  /** Show Diff on a conflict: the file beside the file with the draft in the function's place, in VS Code's diff editor */
  editDiff: { params: EditDiffParams; result: void };
  /** Copy Draft on a function that is gone: the webview's own clipboard may be blocked, the host's is not */
  copyText: { params: { text: string }; result: void };
  addDoc: { params: { key: string }; result: { added: boolean } };
  openDoc: { params: { path: string }; result: void };
  openLink: { params: { href: string; from: string }; result: void };
  goToCode: { params: { key: string }; result: void };
  /** the files under the opened folder that have unsaved changes, as they are now; a canvas asks once when it starts, and after that it listens for the `dirtyFiles` event */
  dirtyFiles: { params: void; result: { paths: string[] } };
  /** the call trees the host holds, by the key of their entry: a canvas that opens later asks once, then listens for the `callTree` event */
  callTrees: { params: void; result: Record<string, CallTreeResult> };
  /** open a file below the opened folder at a range (a call site) */
  openAt: { params: { path: string; range: Range }; result: void };
  /** the session as it is now; a webview asks once when it starts, and after that it listens for the `session` event */
  session: { params: void; result: Session };
  /** Go to Call Site with several sites: a list the host shows, with a preview of each site as it is moved through; the answer is when it closes */
  pickCallSite: { params: { sites: { path: string; range: Range }[] }; result: void };
  /** the bar's ⚠: show the NEEDS ATTENTION view (`vnoc.attention.focus`) */
  focusAttention: { params: void; result: void };
  /** Show Log next to Try Again: reveals the "V-NOC" output channel */
  showLog: { params: void; result: void };
  /** Report Issue next to Show Log, in the sidebar's error state: the host runs `vnoc.reportIssue` */
  reportIssue: { params: void; result: void };
  /** ask the host to fetch an entry's call tree again, after it failed (the footer's Retry) */
  retryCalls: { params: { key: string }; result: void };
  /** a click on a sidebar row (07 §4): the host applies the click rule: the canvas is open → reveal the card there (never a code tab over it); closed → the code as a preview tab */
  rowClick: { params: { context: ContextId; id: string }; result: void };
  /** the buttons of the sidebar's states (06 §8): Open Folder, and Try Again (the core reads the project again) */
  command: { params: { name: "openFolder" | "refreshStructure" }; result: void };
}

/** extension → webview. No answer. */
export interface Events {
  indexChanged: { revision: number };
  docText: { path: string; text: string };
  /** the whole session, every time it changes: whole snapshots cannot get out of step, and `seq` drops one that arrives late */
  session: Session;
  /** a menu entry that changes only what the canvas draws (not the session): show a card's code, fit the view, start an edit of a card (it shows its code first). `checkKeys`, `setDraft` and `save` are for the extension's test suite: it cannot press keys in a webview */
  viewCommand: { name: "showCode" | "hideCode" | "fitView" | "edit" | "checkKeys" | "setDraft" | "save"; key?: string; text?: string };
  /** a call tree arrived: the entry's card shows it in its "Calls" section */
  callTree: { key: string; result: CallTreeResult };
  /** what the host is doing about the call tree of this entry: the canvas draws a spinner for `loading` and a retry for `failed`; `idle` is "nothing in flight" */
  callTreeState: { key: string; state: "loading" | "failed" | "idle"; message?: string };
  /** a probe answered: opening this function would show `count` call cards (0: it calls nothing). `revision` and `epoch` say which index and run of the core */
  callProbe: { key: string; count: number; revision: number; epoch?: number };
  /** the host dropped its probe of this function (a new revision, or nothing draws it any more) */
  callProbeDropped: { key: string };
  /** the host dropped a call tree (stale and nothing wants it, or the oldest unwanted one): the page drops its copy and what it knew of its state */
  callTreeDropped: { key: string };
  /** the core was restarted: every call tree the host held is gone, and so must be the page's (revisions before and after cannot be compared) */
  callTreesReset: Record<string, never>;
  /** the docs tab beside an editor is already open: show this node and stay on it (pinned) */
  showNode: { id: string };
  /** the set of files with unsaved changes changed: `paths` are all of them, relative to the opened folder, sorted (the whole set, as the session is: nothing to get out of step). A card marks its code while its file is in it */
  dirtyFiles: { paths: string[] };
}

/** webview → extension, no answer. */
export type Notes =
  | { type: "intent"; context: ContextId; intent: Intent } // the one way a webview changes the session
  | { type: "rendered"; context: string; visible: number; hidden: number; callCards: number; chevrons: number } // the extension's test suite uses it; `context`: the context the canvas drew; `callCards`: call cards drawn; `chevrons`: function and class cards that show a chevron for their calls
  | { type: "cardsDrawn"; keys: string[]; onScreen: string[] } // `keys`: the function and class cards on the canvas's screen that show nothing of their calls yet, the one to look at first first: the host probes those. `onScreen`: every function and class card on screen, with or without structure children: the host's prefetch of the selected one is for those
  | { type: "chipsDrawn"; chips: { id: string; label: string; active: boolean }[] } // the bar's context chips changed (none: one context): the extension's test suite uses it
  | { type: "draftsDrawn"; count: number; label: string } // the bar's "N unsaved drafts" chip changed (`count` 0, `label` "": no chip): the extension's test suite uses it
  | { type: "warned"; level: "none" | "warning" | "error"; text: string } // the bar's ⚠ changed: the extension's test suite uses it
  | { type: "sidebarRendered"; context: string; rows: number; selected: string | null; inView: boolean; drawn: boolean; height: number } // one panel of the STRUCTURE view drew (`context`: whose panel): how many rows, which one is selected (the active context's, or null), whether the list is scrolled to it (`inView`: read from the element), whether its row is in the page (`drawn`: the virtualizer put it there; a different fact), and how high the list is in px (0: the view is hidden or not laid out, and nothing can be in view). The extension's test suite uses it
  | { type: "codeShown"; key: string; dirty: boolean; text: string } // a card drew the code it read (`key`: the card's id; `text`: as Monaco shows it, the first line's indent taken off the others; `dirty`: whether the card showed its "unsaved in editor" mark when it drew): the extension's test suite uses it
  | { type: "editState"; key: string; state: "closed" | "reading" | "editing" | "saving" | "conflict" | "gone"; elsewhere: boolean } // a card's code is in this state (`key`: the card's id; posted when it shows its code and whenever the state or `elsewhere` changes; `elsewhere`: the card says "Already being edited in another card"): the extension's test suite uses it
  | { type: "keysChecked"; found: boolean; target: string; taken: string[] } // the answer to the viewCommand `checkKeys`: the page dispatched keys at the editor of a code card (`found`: there was one, `target`: its tag), and these were taken by something other than the editor (React Flow's Space, say): the extension's test suite uses it
  | { type: "close" } // the docs tab
  | { type: "error"; message: string }; // a page crashed while drawing: the host logs it, and the test suite fails on it

// What actually travels. `postMessage` carries one of these:
export type RequestMessage = { [M in keyof Requests]: { type: "req"; id: number; method: M; params: Requests[M]["params"] } }[keyof Requests];
export type ResponseMessage = { type: "res"; id: number; result: unknown } | { type: "res"; id: number; error: string };
export type EventMessage = { [E in keyof Events]: { type: "event"; name: E; data: Events[E] } }[keyof Events];
