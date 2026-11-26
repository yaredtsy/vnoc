// What the core answers about tours (docs/26-tours/03-core.md §8, "As built"), as the pages and the host get it. Types only: nothing here runs, and nothing in
// TypeScript computes a state, a count, a next step or whether something changed from a tour (P1, 05-on-screen.md §6): the core answers it.
//
// These are the Rust `--json` types, copied (`crates/vnoc/src/tours/view.rs` is the source of truth: `ListRowView`, `StopView`, `TourView`, `Place`, `OnView`,
// `AsWritten`; `act.rs` and `pin.rs` for the actions). The wire is snake_case, as the rest of the protocol. A key that Rust leaves out when it has nothing to
// say (`skip_serializing_if`) is optional here; every other key is required. `tours.test.ts` reads the Rust goldens and fails when the two differ, so keep
// one property to a line: it reads this file's interfaces to know what they declare.

/** `t-0000`-style id of a tour, as the core makes it (`retry-charge-mjpt`). */
export type TourId = string;
/** `s3`: a stop of one tour. */
export type StopId = string;
/** `s3` (a stop's intro) or `s3.b1`, `s3.b1.b2` (a block, a detail). */
export type StepId = string;

/** `private` is `.vnoc/local/tours/`, `project` is `.vnoc/tours/` (shared by the repository). */
export type TourFolder = "private" | "project";
export type TourState = "writing" | "stopping" | "stopped" | "finished";
export type TourDetail = "quick" | "normal" | "detailed";
/** a kind of node as the tour wrote it; a kind this version does not know is a string too */
export type TourNodeKind = "folder" | "file" | "class" | "function" | (string & {});

/** What the stage draws for a step (design 03 §3, 05 §2). The core decides it; the cover and the end are places, not steps. */
export type StepKind = "intro" | "block" | "detail" | "short" | "contents" | "same_as" | "planned";

/** The first stop of a tour: the row's icon and name. */
export interface RootView {
  kind: string;
  name: string;
}

/** What the cover of a finished tour may offer, by `[tours]` settings: how many written stops have no node ID, and the files of the tour that are not committed. */
export interface OffersView {
  ids: number;
  commit: string[];
}

/** Where the reader is, in a row: the number of the stop on the route (null on the cover and at the end), where, and how many stops were read to their last step. */
export interface PlaceSummary {
  stop: number | null;
  at: "cover" | "step" | "end";
  seen: number;
}

/** With `vnoc/tours { on }`: how a tour is on the node. */
export interface OnView {
  how: "starts" | "stops" | "inside";
  /** the numbers on screen of the stops that are the node */
  stops: number[];
  /** stops inside the node */
  inside: number;
  /** the stop a click opens */
  first: StopId;
  /** the stops of the route */
  of: number;
  /** line 2 of the row, in the core's words */
  words: string;
}

/** One row of the list: `ListRowView`. A tour that cannot be read has `readable: false` and a `problem`, and none of the facts a readable one has. */
export interface TourRow {
  id: TourId;
  folder: TourFolder;
  /** the same id is in the project's folder too: this is the private copy (absent when false) */
  private_copy?: boolean;
  readable: boolean;
  problem?: string;
  title?: string;
  question?: string;
  author?: string;
  state?: TourState;
  written?: number;
  planned?: number;
  steps?: number;
  pinned?: boolean;
  /** the pinned commit, 7 characters or 40 as the core says */
  pin?: string;
  pin_in_repository?: boolean;
  /** the lines of the file that were skipped */
  skipped?: number;
  /** the recorded (or just made) check; absent when none holds */
  check?: "fresh" | "changed";
  on?: OnView;
  minutes?: number;
  /** being written and nothing added for 5 minutes: "no news" */
  quiet: boolean;
  root: RootView | null;
  /** null: nobody opened this tour here ("new") */
  place: PlaceSummary | null;
  offers: OffersView | null;
  /** the row's second line, in the core's words: `shared · ` for a project tour, the one state word that applies, the size, and when it was started (`2 h ago`, `yesterday`, `12 Nov`): `writing · 6 of 10 stops · 2 h ago`, `stop 4 · 9 stops · 2 h ago` */
  words: string;
  /** the one state word `words` starts with (after `shared ·`): `writing`, `changed`, `stop 4`, `finished`, `new`, `seen 2 of 9`; empty for a tour that cannot be read. The On view draws it at the right of line 1 */
  state_word: string;
}

/** The numbers of "stop 3 of 10 · 2 of 5", from the core (the same code `vnoc tour where` uses). */
export interface PositionView {
  /** the stop's place on the route, from 1 */
  stop: number;
  /** how many stops the route has */
  of: number;
  /** the step's place in its stop, from 1 (the intro is 1) */
  step: number;
  steps: number;
}

/** Where the reader is, as the core keeps it (the state file, 01 §9): `at` is `cover`, `end` or a step id. One place per tour per machine (D17). `stop` and `position` are made from the tour as it is now. */
export interface Place {
  at: "cover" | "end" | StepId;
  /** the stops read to their last step, in the order they were */
  seen: StopId[];
  /** where the reader was before each jump, the latest last: `cover`, `end` or a step id */
  back: string[];
  /** when it last moved, `2026-11-23T18:52:10Z`; "" for a place nobody has moved */
  moved: string;
  by: "reader" | "command";
  /** a window is showing this tour (set by `vnoc/tourOpen`) */
  open: boolean;
  /** the current stop's id; null at the cover and at the end */
  stop: StopId | null;
  /** null at the cover and at the end */
  position: PositionView | null;
  /** how many written stops were not read to their last step: the end card says "all seen" or "N not seen" */
  unseen: number;
  /** the first written stop on the route that was not read to its last step; null when every written stop was: the end card's "Go to the first one I skipped" */
  first_unseen: StopId | null;
}

/** A changed or moved block or stop, where its node is now: a changed stop's `now`. */
export interface NowView {
  /** "" for a file */
  qualname: string;
  path: string;
  /** a def's lines; absent for a file */
  lines?: [number, number];
}

/** One block of a stop. */
export interface BlockView {
  id: StepId;
  /** the block a detail is in */
  parent?: StepId;
  /** 0 at the top of the stop, 1 for a detail */
  depth: number;
  lines: [number, number];
  title: string;
  text: string;
  /** with a check: `changed` or `moved` */
  check?: "changed" | "moved";
  /** a moved block's lines now */
  moved_to?: [number, number];
}

/** One step of a stop in reading order: the intro first, then each block with its details. The stage plays these. */
export interface StepView {
  id: StepId;
  kind: StepKind;
  /** 0 the intro, 1 a block, 2 a detail */
  depth: number;
  /** the block a detail is in */
  parent: StepId | null;
  title: string | null;
  /** the intro's text, or the block's (empty for a stop that is planned and not written) */
  text: string;
  /** file lines now; null for an intro */
  lines: [number, number] | null;
  /** with a check: `fresh` or `changed` (a stop that is gone has every step changed) */
  fresh?: "fresh" | "changed";
}

/** A stop's code as the pinned commit has it (`vnoc/tourCode`, and `as_written` of a stop). */
export interface AsWritten {
  /** the first 7 characters of the commit */
  commit: string;
  path: string;
  /** where the stop was at that commit */
  lines: [number, number];
  code: string[];
  /** the lit lines, as ranges of file lines */
  lit: [number, number][];
  /** where each step was at that commit, by step id: the stop's blocks the pin has a place for, and the intro (the stop's id) → the stop's lines */
  steps: Record<string, [number, number]>;
}

/** One stop of the route: `StopView`. */
export interface TourStop {
  /** the number on screen: the place on the route, from 1 */
  number: number;
  id: StopId;
  depth: number;
  /** on the route and not written yet */
  planned: boolean;
  /** null for a planned stop */
  kind: TourNodeKind | null;
  name: string;
  path: string;
  qualname: string;
  /** the node's key in the structure (`TreeNode.id`: a path, or `path::qualname#n`), from what the stop wrote (a planned stop's is where the plan says it will be). null for one a check found gone: the host sets the selection with it, the stage reads the node's code with it */
  key: string | null;
  lines?: [number, number];
  /** the node's permanent ID, when it had one when the stop was written */
  node?: string;
  /** the intro and the blocks, steps in all */
  steps: number;
  from?: StopId;
  line?: number;
  /** for a stop reached by a call: the step of the `from` stop that holds the line (the deepest block, else its intro): "called on line N" jumps there */
  line_step?: StepId;
  same_as?: StopId;
  text?: string;
  /** with a check: `changed`, `gone`, or `moved`; a fresh stop says nothing */
  check?: "changed" | "gone" | "moved";
  now?: NowView;
  as_written?: AsWritten;
  blocks: BlockView[];
  /** the stop's steps in reading order, the intro first */
  reading: StepView[];
}

/** What a check said of the whole tour. */
export interface CheckInfo {
  fresh: boolean;
  /** `record`, `check`, or `memory` (the answer the core gave last time) */
  from: "record" | "check" | "memory";
  at: string;
  changed: number;
  gone: number;
}

/** A tour as `vnoc/tour` answers it: `TourView`. The CLI's `show --json` adds `schema`, `command` and `readable` around the same object. */
export interface TourView {
  id: TourId;
  title: string;
  question?: string;
  author?: string;
  detail: TourDetail;
  state: TourState;
  folder: TourFolder;
  written: number;
  planned: number;
  steps: number;
  minutes: number;
  pinned: boolean;
  pin?: string;
  pin_in_repository?: boolean;
  check?: CheckInfo;
  stops: TourStop[];
  /** the planned stop the agent is on, while the tour is being written */
  writing: StopId | null;
  /** being written (`writing` or `stopping`) and the tour's file untouched for 5 minutes: "no news" */
  quiet: boolean;
  /** the size of the route, `written + planned`; null while the tour is being written with no plan (the number is not known yet) */
  of: number | null;
  /** where the reader is: the core's, kept per tour */
  place: Place;
  offers: OffersView | null;
  /** `line 7: why`: lines of the file that could not be read */
  problems: string[];
}

/** `vnoc/tour` answers a tour that cannot be read with its reason. */
export interface TourUnreadable {
  unreadable: string | null;
}

/** `vnoc/tourHalt`. */
export interface TourHaltAnswer {
  halt: "written" | "already" | "finished";
  state: TourState;
}

/** `vnoc/tourCounts`: how many tours pass through each node, by the structure's key. */
export interface TourCountsAnswer {
  counts: Record<string, number>;
}

/** `vnoc/tourCalls`: for each written function or class stop of a tour, how many of the functions it calls are not a stop of the tour (the card's `+N`). A stop with none is not in it. */
export interface TourCallsAnswer {
  off: Record<StopId, number>;
}

export interface PinChanged {
  /** git's two-letter status: ` M`, `M `, `??` … */
  status: string;
  path: string;
}

/** Why a tour is not pinned (`code` says which; the rest of the keys depend on it). */
/** Why a tour is not pinned. Each reason says itself, in the core's words (`words`, one sentence, no full stop): a page makes no sentence from a `code`. */
export type PinReason = { words: string } & (
  | { code: "unfinished" | "no_stops" | "not_a_repository" | "git_not_found" | "no_commit" }
  | { code: "in_progress"; what: string }
  | { code: "hook_changed"; path: string; tour: TourId }
  | { code: "stale"; changed: StopId[]; gone: StopId[] }
  | { code: "uncommitted"; files: PinChanged[] }
  | { code: "not_held"; path: string; ignored: boolean }
  | { code: "submodule"; path: string; module: string }
  | { code: "git_failed" | "not_checked"; why: string }
);

export interface CommittedOut {
  commit: string;
  message: string;
  /** the tour's code files */
  files: string[];
  node_files?: string[];
}

/** `vnoc/tourPin`'s parameters: `commit` commits the files that are the one thing in the way first; `dry` answers as a pin would and writes nothing (not with `commit`). */
export interface TourPinParams {
  id: TourId;
  commit?: { message?: string };
  dry?: boolean;
}

/** `vnoc/tourPin`. */
export interface TourPinAnswer {
  tour: TourId;
  state: "pinned" | "ask" | "not_pinned";
  /** the pinned commit, all 40 characters */
  commit?: string;
  committed?: CommittedOut;
  reasons?: PinReason[];
  /** for `ask`: the command that would do it */
  command?: string;
  node_files?: string[];
  /** `--commit` was asked and was refused: nothing was committed */
  refused?: boolean;
  /** pinned at a commit HEAD has behind it: nothing was done */
  already?: boolean;
}

export interface IdStop {
  stop: StopId;
  state: "written" | "had" | "by_name" | "gone";
  id?: string;
  /** the file the ID line is in, or the `.vnoc-id` file of the folder */
  changed?: string;
  said?: string;
}

/** `vnoc/tourIds`: it writes into the files on disk (TR22 saves a dirty buffer of them first). */
export interface TourIdsAnswer {
  schema: number;
  command: string;
  tour: TourId;
  stops: IdStop[];
  /** every file written or made, once, in the order they were */
  changed: string[];
  recorded: boolean;
  /** the core's sentence, shown as it is: *"IDs written for 2 stops in 1 file."* */
  words: string;
}

export type SettingChoice = "always" | "ask" | "never";

export interface SettingShown {
  value: SettingChoice;
  /** `file` (`[tours]` in `.vnoc/vnoc.toml` says it) or `default` */
  from: "file" | "default";
}

/** `vnoc/tourSettings`. */
export interface TourSettingsAnswer {
  schema: number;
  command: string;
  ids: SettingShown;
  commit: SettingShown;
  /** the keys this call changed */
  changed?: string[];
  problems?: string[];
  /** `ids = "always"` in a project that writes no ID lines: it does nothing there */
  does_nothing?: string;
}

/** `vnoc/tourEdit`: what was set, and the core's sentence for it. */
export interface TourEditAnswer {
  tour: TourId;
  title: string | null;
  question: string | null;
  words: string;
}

/** `vnoc/tourShare`. */
export interface TourShareAnswer {
  tour: TourId;
  folder: TourFolder;
  /** the file, relative to the git root */
  path: string;
  /** the tour is pinned: a project tour that is not has no old code for teammates, and the message offers **Pin** */
  pinned: boolean;
  /** the core's sentence: *"Moved to `.vnoc/tours/`. Commit it to share it."* */
  words: string;
}

/** `vnoc/tourDelete`: the trash entry `vnoc/tourRestore` takes. */
export interface TourDeleteAnswer {
  token: string;
  /** *"Deleted “How the shop checks out”."*: the row says it, with **Undo** after it */
  words: string;
}

/** `vnoc/tourDeleteFinished`: each private finished tour that went to the trash, with its token; and the core's sentence for all of them. */
export interface TourDeleteFinishedAnswer {
  deleted: { id: TourId; token: string }[];
  words: string;
}

/** `vnoc/tourRestore`. */
export interface TourRestoreAnswer {
  tour: TourId;
  folder: TourFolder;
}

/** What `move` may be in `vnoc/tourGo`: the words of `vnoc tour go`. A jump is `s4`, `s3.b2`, `cover` or `end`; the core makes the move and answers the new place. */
export type TourMove = "next" | "prev" | "next-stop" | "prev-stop" | "back" | (string & {});

export interface ToursParams {
  /** a tree key (or any reference the core reads): the tours that pass through that node, each row with `on` */
  on?: string;
  /** the tours to check now */
  check?: TourId[];
  /** **Check All Tours**: every tour is checked anew; the rows say what the check found */
  check_all?: boolean;
  /** a tree key (or a reference): every tour, the ones on that node first, each with `on` (**Open Tour…**'s order) */
  first_on?: string;
}

/** One thing wrong with an imported document: its place as a JSON path (`stops[0].blocks[1].text`), the core's sentence, and the line of the text when the core knows it. */
export interface TourRefusal {
  place: string;
  words: string;
  line: number | null;
}

/** `vnoc/tourImport`: the tour that was written, or every refusal of the document (nothing was written); `words` is the core's sentence either way. */
export type TourImportAnswer = { id: TourId; words: string; refused?: undefined } | { refused: TourRefusal[]; words: string; id?: undefined };

/** How far the form's tour goes from its node: `agent` says nothing in the sentence. */
export type AskReach = "agent" | "only" | "calls" | "calls2";
/** How much each step says: `agent` says nothing in the sentence. */
export type AskDetail = "agent" | "quick" | "normal" | "detailed";

/** The parts of the sentence for an agent, as the form of **Ask for a Tour…** has them (`vnoc/tourAsk`): the core makes the sentence from them. With `tour`, that tour's root, question and detail fill what the others do not give. */
export interface TourAskParams {
  key?: string;
  question?: string;
  reach?: AskReach;
  detail?: AskDetail;
  tour?: TourId;
}

/** `vnoc/tourAsk`: the sentence for the agent, and the line that says what to do with it. With `tour`, `filled` is what that tour gave the form: its root's address, its question, its detail. */
export interface TourAskAnswer {
  text: string;
  words: string;
  filled?: { key?: string; question?: string; detail: AskDetail };
}

export interface ToursAnswer {
  /** newest first, in the core's order */
  tours: TourRow[];
}

/** Every request to the core about tours, by its method (03 §8). The host sends them; a page never does. */
export interface TourCoreRequests {
  "vnoc/tours": { params: ToursParams | undefined; result: ToursAnswer };
  "vnoc/tour": { params: { id: TourId }; result: TourView | TourUnreadable };
  "vnoc/tourGo": { params: { id: TourId; move: TourMove }; result: Place };
  "vnoc/tourOpen": { params: { id: TourId; open: boolean }; result: Place };
  "vnoc/tourCode": { params: { id: TourId; stop: StopId }; result: AsWritten | null };
  "vnoc/tourHalt": { params: { id: TourId }; result: TourHaltAnswer };
  "vnoc/tourPin": { params: TourPinParams; result: TourPinAnswer };
  "vnoc/tourIds": { params: { id: TourId }; result: TourIdsAnswer };
  "vnoc/tourEdit": { params: { id: TourId; title?: string; question?: string }; result: TourEditAnswer };
  "vnoc/tourShare": { params: { id: TourId; share: boolean }; result: TourShareAnswer };
  "vnoc/tourDelete": { params: { id: TourId }; result: TourDeleteAnswer };
  "vnoc/tourDeleteFinished": { params: undefined; result: TourDeleteFinishedAnswer };
  "vnoc/tourRestore": { params: { token: string }; result: TourRestoreAnswer };
  "vnoc/tourSettings": { params: { ids?: SettingChoice; commit?: SettingChoice } | undefined; result: TourSettingsAnswer };
  "vnoc/tourExport": { params: { id: TourId }; result: { text: string } };
  "vnoc/tourImport": { params: { text: string; replace?: TourId }; result: TourImportAnswer };
  "vnoc/tourAsk": { params: TourAskParams | undefined; result: TourAskAnswer };
  "vnoc/tourCounts": { params: undefined; result: TourCountsAnswer };
  "vnoc/tourCalls": { params: { id: TourId }; result: TourCallsAnswer };
}

/** The two notifications the core sends (03 §8). */
export interface TourCoreNotifications {
  /** a tour's file changed, appeared or went; or the answer the core gave about its freshness no longer holds: ask again for what is shown */
  "vnoc/toursChanged": { ids: TourId[] };
  /** the place was moved by another process (`vnoc tour go`); not sent for the core's own `vnoc/tourGo` */
  "vnoc/tourPlaceChanged": { id: TourId; place: Place };
}
