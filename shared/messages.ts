// Everything that crosses between the extension host and a webview, declared once. A message that is not in this file does not exist
// (docs/10-ui-rework/01-state.md §4). Types only: nothing here runs, so any side can import it.

import type { ContextId, Intent, Session } from "./session";
import type { Place, TourId, TourMove, TourRow, TourUnreadable, TourView } from "./tours";

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

/**
 * A documented node whose `ID:` line was lost: `node.toml` says the ID is in the code and the code has none, so the node was found by its name. It is in the
 * tree, and *Write ID* puts the line back (`vnoc.writeId` takes `key`).
 */
export interface LostMarker {
  /** the permanent ID */
  id: string;
  /** the node's id in the tree: `path::qualname[#n]` for a def, the path for a file or a folder */
  key: string;
  kind: Kind;
  path: string;
  /** empty for a file or a folder */
  qualname: string;
}

/** Something to look at: where to go when it is clicked. */
export interface Problem {
  text: string;
  /** absolute path */
  file: string | null;
  /** 0-based */
  line: number | null;
}

/** One entry of `vnoc/languages`: a driver the core was given and what it said. The extension has no list of languages of its own: this is it. */
export interface LanguageInfo {
  /** the driver's own name; for one that never answered, the name of its file without `vnoc-driver-` */
  name: string;
  version: string;
  path: string;
  /** the file extensions it claimed, without the dot */
  extensions: string[];
  /** which optional methods it has (`callTree`, …) */
  capabilities: Record<string, boolean>;
  state: "running" | "failed";
  error?: string;
  warnings?: string[];
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
  /** nodes found by name whose `ID:` line was lost, sorted by path. An older core sends none: read an absent field as `[]` */
  lost_markers?: LostMarker[];
}

/** `vnoc/search`: one node the query found. */
export interface SearchNode {
  /** the tree's id */
  id: string;
  kind: Kind;
  name: string;
  /** relative to the opened folder */
  path: string;
  /** 0-based; null for a file or a folder */
  line: number | null;
  /** a function inside a class: a method */
  in_class: boolean;
  /** how many docs are attached */
  docs: number;
  /** 0 to 5, smaller is better */
  tier: number;
  /** the node a pasted reference named */
  exact: boolean;
  /** the parts of `name` to draw bold: [start, end] in UTF-16 units, sorted and merged */
  match: [number, number][];
}

/** `vnoc/search`: one doc the query found. The core sends none until the doc text is searched (S6). */
export interface SearchDoc {
  /** the tree's node id; null for a Missing node */
  node: string | null;
  node_id: string;
  node_name: string;
  node_kind: Kind | null;
  node_path: string;
  missing: boolean;
  file: string;
  doc_path: string;
  title: string;
  title_match: [number, number][];
  snippet: string;
  snippet_match: [number, number][];
  /** the doc's frontmatter `tags:`, lower case, in the order written. Left out by a core older than A8b, and empty when the doc has none. */
  tags?: string[];
  /** the doc's frontmatter `author:` */
  author?: string | null;
}

export interface SearchResult {
  /** the index revision this answer is from (the same one as `StructureResult.revision`) */
  revision: number;
  nodes: SearchNode[];
  /** how many nodes matched before the limit */
  nodes_total: number;
  docs: SearchDoc[];
  docs_total: number;
  /** what was wrong with a filter in the query */
  notes: string[];
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
  /** the doc's frontmatter `tags:`, lower case, in the order written. Left out by a core older than A8b, and empty when the doc has none. */
  tags?: string[];
  /** the doc's frontmatter `author:` */
  author?: string | null;
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

/** The two pages that hold drafts: the canvas, and the docs tab beside an editor (docs/17-edit-in-canvas.md §8, 9.6). Both are the one bundle; `data-mode` says which. */
export type Page = "canvas" | "docs";

/** A draft's state (docs/17-edit-in-canvas.md §4): `editing`, `saving` (a Save is asking the host), `conflict` (the function changed in the editor), `gone` (it is not in the file any more). */
export type DraftState = "editing" | "saving" | "conflict" | "gone";

/**
 * One function's unsaved edit (§4, E6), as the page holds it and as the host keeps its copy (§7, 9.5c). `key` is the structure node's key (a call card's callee's), so two cards
 * on one function have one draft. `card` is the id of the card that started it: the only card that shows it editing. `path`, `range`, `version`, `base`, `prefix` and `eol` are
 * what the host said when it started: §6's checks compare against them at save time. `text` is what Monaco holds, the first line's indent taken off the others
 * (`dedent(base, prefix)` when nothing was typed).
 *
 * `kind: "doc"` (§8, 9.6): the draft of a doc's Markdown file, not of a function. Its `key` and `path` are the file's absolute path, its `range` is the whole file, its `prefix` is `""`, and
 * its `card` is the node the panel showed it for (what the page shows again to bring the draft back). Everything else is the same draft.
 */
export interface Draft {
  kind?: "doc";
  key: string;
  card: string;
  path: string;
  range: Range;
  version: number;
  base: string;
  prefix: string;
  /** the document's line ending, as the baseline said; a draft saved before it was there has `"\n"` */
  eol: "\n" | "\r\n";
  text: string;
  state: DraftState;
}

/**
 * What a card's code is, as the host reads it (docs/17-edit-in-canvas.md §4, §5): the text of the range, the document's `version`, the `prefix` (the first line's text before
 * `range.start`: the `def`'s own indentation, as written), `eol` (the document's line ending, which `reindent` joins the written lines with) and whether the file has unsaved
 * changes in a tab (`dirty`). The card keeps it as its **baseline**: the first keystroke that makes the text differ from it starts a draft from it.
 */
export interface CodeResult {
  text: string;
  version: number;
  prefix: string;
  eol: "\n" | "\r\n";
  dirty: boolean;
}

/**
 * What the doc panel's pencil answers (§8): the same baseline for a whole doc, with the whole file as the range as the host reads it now. Or that the `.md` has unsaved changes in
 * a tab (`dirty`), or that the other page has a draft of it (`busy`): one draft per doc, across the canvas and the docs tab.
 */
export type EditStartResult = { dirty: true } | { busy: true } | { text: string; version: number; prefix: string; eol: "\n" | "\r\n"; range: Range };

/** What the pencil sends: the Markdown file's absolute path (`DocInfo.path`, which the host checks lies in `<git root>/.vnoc/nodes/`), and the page that asks, so the host can say the other page has it. */
export type EditStartParams = { kind: "doc"; path: string; saveFirst?: boolean; page: Page };

/** What a draft sends to be written: where it was, what it started from (`version`, `base`), how to put the indent and the line endings back, and the text. */
export interface EditSaveParams {
  /** `"doc"`: `path` is a doc's absolute path and `range` is the whole file; Save runs the same rows with a `locate` that answers the whole document, and never asks about an ID (§8) */
  kind?: "doc";
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
  kind?: "doc";
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
  /** the sidebar's box (docs/20-search): the core finds and ranks, the page draws. `nodes`: at most this many rows (the core says 200 when absent) */
  search: { params: { query: string; nodes?: number }; result: SearchResult };
  /** `vnoc/searchCards`: of these node ids (a structure card's id, a call card's target key), which the query finds, by name or by a doc of the node; `hits` is a subset, in the order given. The canvas dims the rest */
  searchCards: { params: { query: string; ids: string[] }; result: { revision: number; hits: string[] } };
  /** the permanent ID (`TreeNode.node_id`), not the location key; the relay renames it for the core */
  docs: { params: { nodeId: string }; result: DocInfo[] };
  docText: { params: { path: string }; result: string };
  /**
   * A card's code and its baseline (see `CodeResult`), from VS Code's document (so unsaved edits show). `saveFirst` saves a document with unsaved changes first (the card's **Save** in
   * "Save `<file>` to edit here"): the range is the one the card sent, which is stale until the index has read the save, so the card waits for it before it trusts the answer.
   */
  code: { params: { path: string; range: Range; saveFirst?: boolean }; result: CodeResult };
  /**
   * The doc panel's pencil (§8): the host opens the doc. Dirty (and no `saveFirst`): `{ dirty: true }`, nothing else; the other page has a draft of it: `{ busy: true }`. Otherwise
   * the baseline of the whole file. `saveFirst` saves a dirty document first ("Save and Edit"); there is no range to wait for, so it answers at once. A function's code has no such
   * request: its baseline is the answer to `code`.
   */
  editStart: { params: EditStartParams; result: EditStartResult };
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
  /**
   * A doc row of the search was picked (docs/20-search/05-on-screen.md §1). `node`: the tree's node id, null when the node is not in the tree; `nodeId`: the permanent ID; `missing`:
   * the node is Missing; `path`: the doc. A node in the tree: the page has revealed it, and the host shows the docs tab pinned to it with that doc chosen. Missing: the tab pinned to
   * the Missing node (`MISSING_PIN` + `nodeId`), that doc chosen. Neither (its code was not read): there is nothing to pin, so the `.md` opens in an editor tab.
   */
  showDoc: { params: { node: string | null; nodeId: string; missing: boolean; path: string }; result: void };
  openLink: { params: { href: string; from: string }; result: void };
  goToCode: { params: { key: string }; result: void };
  /** the files under the opened folder that have unsaved changes, as they are now; a canvas asks once when it starts, and after that it listens for the `dirtyFiles` event */
  dirtyFiles: { params: void; result: { paths: string[] } };
  /**
   * the host's copy of the drafts (§7, 9.5c), as the last `drafts` note left it; a canvas asks once when it starts, and takes them only when its own saved state has none (a
   * webview reload keeps its own). Reading them does not clear them: only the page's empty list, Discard and a Save All that handled them do
   */
  drafts: { params: { page: Page }; result: { drafts: Draft[] } };
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
  /**
   * The list of tours (`vnoc/tours`), in the core's order. With `on`, a structure key: the tours that pass through that node, each row with its `on`. The host holds the
   * plain list and sends it again as the `tours` event when the core says a tour changed; a page that starts asks once.
   */
  tours: { params: { on?: string } | void; result: TourRow[] };
  /** A tour as the stage plays it (`vnoc/tour`), with the place as the core has it now. The host holds the open tour's and answers from it; `tour` events follow. */
  tour: { params: { id: TourId }; result: TourView | TourUnreadable };
  /** A move in a tour (`vnoc tour go`'s words: `next`, `prev`, `next-stop`, `prev-stop`, `back`, `s4`, `s3.b2`, `cover`, `end`): the core makes it and answers the new place, which the host takes */
  tourGo: { params: { id: TourId; move: TourMove }; result: Place };
}

/** extension → webview. No answer. */
export interface Events {
  indexChanged: { revision: number };
  docText: { path: string; text: string };
  /** the whole session, every time it changes: whole snapshots cannot get out of step, and `seq` drops one that arrives late */
  session: Session;
  /** a menu entry that changes only what the canvas draws (not the session): show a card's code, fit the view, start an edit of a card (it shows its code first). `checkKeys`, `setDraft` and `save` are for the extension's test suite: it cannot press keys in a webview. `saveFile` is the Save button of "Save `<file>` to edit here" on a card; `showDocPanel` opens the canvas's doc panel; `docEdit`, `docSetDraft` and `docSave` (`key`: the doc's path) are Edit, typing and Save in a doc panel, on whichever page shows that doc */
  viewCommand: { name: "showCode" | "hideCode" | "fitView" | "edit" | "checkKeys" | "setDraft" | "save" | "showDocPanel" | "docEdit" | "docSetDraft" | "docSave" | "saveFile" | "setSearch" | "focusSearch" | "searchKey" | "stageKey" | "stageClick" | "toursClick" | "setView" | "mapClick"; key?: string; text?: string };
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
  /** the docs tab beside an editor is already open: show this node and stay on it (pinned); `path`: and show this doc of it (a search pick) */
  showNode: { id: string; path?: string };
  /** the set of files with unsaved changes changed: `paths` are all of them, relative to the opened folder, sorted (the whole set, as the session is: nothing to get out of step). A card marks its code while its file is in it */
  dirtyFiles: { paths: string[] };
  /** the list of tours, again (the core said a tour changed, and a page asked for the list before): the whole list, in the core's order */
  tours: { tours: TourRow[] };
  /** the open tour, again: it changed, or its place did (the host took the core's answer). The whole view, with the place in it; `unreadable` when the file cannot be read now. `null`: no tour is open */
  tour: { id: TourId; view: TourView | TourUnreadable } | { id: null; view: null };
}

/** webview → extension, no answer. */
export type Notes =
  | { type: "intent"; context: ContextId; intent: Intent } // the one way a webview changes the session
  | { type: "rendered"; context: string; visible: number; hidden: number; callCards: number; chevrons: number } // the extension's test suite uses it; `context`: the context the canvas drew; `callCards`: call cards drawn; `chevrons`: function and class cards that show a chevron for their calls
  | { type: "cardsDrawn"; keys: string[]; onScreen: string[] } // `keys`: the function and class cards on the canvas's screen that show nothing of their calls yet, the one to look at first first: the host probes those. `onScreen`: every function and class card on screen, with or without structure children: the host's prefetch of the selected one is for those
  | { type: "chipsDrawn"; chips: { id: string; label: string; active: boolean }[] } // the bar's context chips changed (none: one context): the extension's test suite uses it
  | { type: "drafts"; page: Page; drafts: Draft[] } // a page's drafts (`page`: the canvas, or the docs tab: the host merges both into one copy), the whole list: the host keeps a copy so a closed canvas does not lose them (§7, 9.5c). At once when a draft starts, changes state or goes; 500 ms after the last keystroke for typing; at once when the page is hidden
  | { type: "draftsDrawn"; count: number; label: string } // the bar's "N unsaved drafts" chip changed (`count` 0, `label` "": no chip): the extension's test suite uses it
  | { type: "warned"; level: "none" | "warning" | "error"; text: string } // the bar's ⚠ changed: the extension's test suite uses it
  | { type: "sidebarRendered"; context: string; rows: number; selected: string | null; inView: boolean; drawn: boolean; height: number } // one panel of the STRUCTURE view drew (`context`: whose panel): how many rows, which one is selected (the active context's, or null), whether the list is scrolled to it (`inView`: read from the element), whether its row is in the page (`drawn`: the virtualizer put it there; a different fact), and how high the list is in px (0: the view is hidden or not laid out, and nothing can be in view). The extension's test suite uses it
  | { type: "codeShown"; key: string; dirty: boolean; text: string } // a card drew the code it read (`key`: the card's id; `text`: as Monaco shows it, the first line's indent taken off the others; `dirty`: whether the card showed its "unsaved in editor" mark when it drew): the extension's test suite uses it
  | { type: "editState"; key: string; state: "closed" | "reading" | "editing" | "saving" | "conflict" | "gone"; elsewhere: boolean; mark: string | null } // a card's code is in this state (`key`: the card's id; posted when it shows its code and whenever the state or `elsewhere` changes; `elsewhere`: the card says "Being edited in another card", or the doc panel "Already being edited in the canvas"; `mark`: which line is above the code, `saveFirst`, `saving`, `elsewhere`, `state`, `notice`, `refusal`, `busy`, or null): the extension's test suite uses it
  | { type: "keysChecked"; found: boolean; target: string; taken: string[]; hosts: number } // the answer to the viewCommand `checkKeys`: the page dispatched keys at the editor of a code card (`found`: there was one, `target`: its tag, `hosts`: how many code areas the page had, editor made or not), and these were taken by something other than the editor (React Flow's Space, say): the extension's test suite uses it
  | { type: "searchDrawn"; query: string; ids: string[]; docs: string[]; total: number; notes: string[]; ms: number; trip: number } // the STRUCTURE view drew the rows of an answer (`query`: the box's text it answers, `ids`: the node rows in order, `docs`: the doc rows' `doc_path`s in order, `total`: how many matched before the limit, `notes`: the lines drawn under the box about its filters, `ms`: from the change of the box to the draw, `trip`: from sending the request to the draw): the extension's test suite uses it
  | { type: "docsShown"; pinned: string | null; node: string | null; doc: string | null } // the docs tab shows this: `pinned`: the node it is held on (a Missing node's carries `MISSING_PIN`), `node`: the node it draws, `doc`: the doc's path (null: none yet): the extension's test suite uses it
  | { type: "searchDim"; query: string; dimmed: string[] } // the canvas applied an answer of `searchCards` (`query`: the text it answers, `dimmed`: the ids of the cards it dimmed, the selected one never among them; a call card with no key is dimmed), and once with `dimmed: []` when the search ended: the extension's test suite uses it
  | { type: "searchFocused"; focused: boolean; text: string } // the STRUCTURE view handled the view command `focusSearch` (*V-NOC: Search…*): `focused`: the cursor is in the box now, `text`: what the box holds: the extension's test suite uses it
  | { type: "stageDrawn"; tour: string; mode: "cover" | "step" | "end"; stop: string | null; step: string | null; kind: string | null; header: string; title: string | null; text: string | null; position: string; next: string; unseen: number; calls: number[]; code: { first: number; text: string } | null; lit: number[]; thin: number[]; scrolledTo: number } // the tour's stage drew this (`header`: the header's words, `position`: "stop 3 of 10 · 2 of 5", `next`: the primary button's word, `unseen`: the core's count the end card says, `calls`: the file lines of the code that lead to a later stop (a jump), `code`: the pane's text and the file line it starts at, `lit` and `thin`: the file lines drawn lit and with the thin mark, `scrolledTo`: the line at the top of the pane, from 0): the extension's test suite uses it
  | { type: "toursDrawn"; folded: boolean; mode: "list" | "route"; count: number; header: string; list: { id: string; title: string; words: string; question: string | null; active: boolean; clickable: boolean }[]; route: { id: string; kind: "stop" | "step"; text: string; mark: string | null; current: boolean; number: number | null; description: string | null; depth: number }[] } // the sidebar's TOURS section drew this (`mode`: the list or the open tour's route, `count`: the rows it has, `header`: the route header's words, `list`: each row's title and the core's `words`, `route`: each stop and step with its mark): the extension's test suite uses it
  | { type: "mapDrawn"; tour: string; cards: { id: string; number: number; mark: string; here: boolean; callLine: number | null; description: string | null; planned: boolean }[]; heavy: string[]; edges: string[] } // the tour's map drew these cards (`mark`: seen / here / ahead / planned, `here`: the ring, `callLine`: a call card's line), the ids of the heavy edges, and every edge as `source->target:call|node`: the extension's test suite uses it
  | { type: "viewport"; context: string; x: number; y: number; zoom: number } // a canvas was panned or zoomed (the view `context` now has): the extension's test suite uses it
  | { type: "close" } // the docs tab
  | { type: "error"; message: string }; // a page crashed while drawing: the host logs it, and the test suite fails on it

// What actually travels. `postMessage` carries one of these:
export type RequestMessage = { [M in keyof Requests]: { type: "req"; id: number; method: M; params: Requests[M]["params"] } }[keyof Requests];
export type ResponseMessage = { type: "res"; id: number; result: unknown } | { type: "res"; id: number; error: string };
export type EventMessage = { [E in keyof Events]: { type: "event"; name: E; data: Events[E] } }[keyof Events];
