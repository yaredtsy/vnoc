import * as path from "path";

import type { Events } from "@vnoc/shared/messages";
import type { ContextId, ContextState, Intent, Session } from "@vnoc/shared/session.ts";
import { isWriting, statusBarText, toAnnounce } from "@vnoc/shared/tourWriting.ts";
import type { AsWritten, Place, StopId, TourCallsAnswer, TourCoreRequests, TourHaltAnswer, TourId, TourMove, TourRow, TourUnreadable, TourView } from "@vnoc/shared/tours.ts";

// The host's holder of tours (docs/26-tours/05-on-screen.md §5, §6). It asks the core, holds the open tour's `TourView` with its place, passes it to the pages, and sets
// the tour context's selection from the place. It computes nothing about a tour (P1): which stop is current, what comes next, whether anything changed are the core's
// answers. No `vscode` in this file: what the host gives it is in `ToursDeps`, so the decisions can be tested with `node --test`.

/** How often the open tour is asked for again when `.vnoc/` is above the opened folder (open check 11): the core answers from two files' lengths and times. */
export const REREAD_MS = 2000;

/** How often the open tour is asked for again while it is being written: `quiet` ("no news") is the file's age, and nothing changes in a file nobody writes, so no `vnoc/toursChanged` says it. */
export const QUIET_CHECK_MS = 30_000;

/** What the holder needs from the host. */
export interface ToursDeps {
  /** a request to the core about tours (`TourCoreRequests` has each method's params and answer) */
  request<M extends keyof TourCoreRequests>(method: M, params: TourCoreRequests[M]["params"]): Promise<TourCoreRequests[M]["result"]>;
  session(): Session;
  /** an intent for a context of the session */
  dispatch(intent: Intent, context: ContextId): void;
  /** to every open page */
  emit<E extends "tours" | "tour">(name: E, data: Events[E]): void;
  /** `.vnoc/` is above the opened folder (`vnocIsAbove`): the host's file watcher cannot see it */
  vnocAbove(): boolean;
  /** runs `run` every `ms` until stopped */
  every(ms: number, run: () => void): { stop(): void };
  log(text: string): void;
  /** the status bar item: its text, or null to hide it (`▶ Tour 3/10`, design 04 §6). A click on it shows the canvas */
  statusBar(text: string | null): void;
  /** the one notification (W11): a tour started being written and it was not opened for the reader. It has **Open Tour**; nothing opens by itself */
  announce(row: TourRow): void;
  /** the canvas is open and in front, whatever its active context is */
  canvasVisible(): boolean;
  /** makes the tour the window's tour context, active, at the cover (W11: the canvas is visible and the window has no tour open) */
  open(row: TourRow): void;
}

/**
 * What a tour that appears and is being written does (W11, design 02 frame 2, design 05 §5): `open` its context when the canvas is visible and no tour is open; else `notify`, once per
 * tour, whatever the canvas shows (a second tour while one is open is announced, and the open tour stays).
 */
export type Appearance = "open" | "notify";

export function appearance(canvas: { visible: boolean; tourOpen: boolean }): Appearance {
  return canvas.visible && !canvas.tourOpen ? "open" : "notify";
}

/** The ids of the rows a list can ask the core to check: the ones that can be read, each once (a shared tour and its private copy are two rows with one id). */
export function readableIds(rows: readonly TourRow[]): TourId[] {
  return [...new Set(rows.filter((r) => r.readable).map((r) => r.id))];
}

// ── the decisions that need no host ──────────────────────────────────────────────────────────────────────────────────────

/**
 * What `vnoc/toursChanged { ids }` makes the holder ask again: the open tour when it is named, and the list whenever one was held (any tour may be a row of it). A
 * change to a tour nobody has open and no page listed asks nothing.
 */
export function whatToAsk(ids: readonly TourId[], open: TourId | null, listHeld: boolean): { tour: boolean; list: boolean } {
  return { tour: open !== null && ids.includes(open), list: listHeld };
}

/**
 * Whether `.vnoc/` is above the opened folder: V-NOC's folder is at the git root, and the host's watcher only sees what is below the folder that was opened. So it is
 * when the git root is a folder above the opened one. Unknown until the core has said where the git root is (the first structure): not above.
 */
export function vnocIsAbove(openedFolder: string | undefined, gitRoot: string | undefined): boolean {
  if (!openedFolder || !gitRoot) return false;
  const way = path.relative(gitRoot, openedFolder);
  return way !== "" && !way.startsWith("..") && !path.isAbsolute(way);
}

/** The open tour is asked for again every `REREAD_MS` only while one is open and the watcher cannot see its files. */
export const shouldPoll = (open: boolean, vnocAbove: boolean): boolean => open && vnocAbove;

/** The open tour is asked for again every `QUIET_CHECK_MS` only while it is being written (`writing` or `stopping`): that is when a tour can go quiet. */
export const shouldCheckQuiet = (openState: TourView["state"] | null): boolean => openState !== null && isWriting({ state: openState });

/**
 * The key of the node the tour context selects for a place the host holds: the current stop's (`place.stop`, the core's), the first stop's at the cover and at the end.
 * `null` when that stop has no node in the tree (planned, or gone).
 */
export function selectionKey(view: TourView): string | null {
  const stop = view.place.stop === null ? view.stops[0] : view.stops.find((s) => s.id === view.place.stop);
  return stop?.key ?? null;
}

/**
 * One row of Open Tour…'s QuickPick (design 02 frame 10), in the core's own words and in its order: the label is the tour's title (its id for a tour that cannot be
 * read), the description the row's second line (`words`: the state word and the size), the detail the question the tour answers, or why it cannot be read.
 */
export function pickItem(row: TourRow): { label: string; description: string; detail?: string; row: TourRow } {
  return { label: row.title ?? row.id, description: row.words, detail: row.readable ? row.question : row.problem, row };
}

/** The context of the session that shows a tour: at most one. */
export const tourContext = (s: Session): ContextState | undefined => s.order.map((id) => s.contexts[id]).find((c) => c.kind === "tour");

// ── the holder ───────────────────────────────────────────────────────────────────────────────────────────────────────────

type Answer = TourView | TourUnreadable;

export class Tours {
  /** the tour a window shows, as the core last said it; `view` is null until the first answer */
  private open: { id: TourId; view: Answer | null; raw: string } | null = null;
  /** the plain list, once a page asked for it: the core's order, held so that a change can send it again */
  private rows: TourRow[] | null = null;
  /** the ids of the last list, for the tours that appear: null until a list was held (a window that starts with tours announces none of them) */
  private known: Set<TourId> | null = null;
  /** the tours the notification was shown for: once per tour */
  private announced = new Set<TourId>();
  private poll: { stop(): void } | undefined;
  private quietCheck: { stop(): void } | undefined;
  /** the last text given to the status bar item (undefined: none yet), so that it is told only what changed */
  private status: string | null | undefined;

  private readonly host: ToursDeps;

  constructor(host: ToursDeps) {
    this.host = host;
  }

  /** The tour the window shows now, from the session; the pages and the core are told when it changed. Called after every change of the session, and once at the start. */
  sync(): void {
    const want = tourContext(this.host.session())?.tour ?? null;
    const have = this.open?.id ?? null;
    if (want === have) return;
    if (have !== null) void this.tell(have, false);
    this.open = want === null ? null : { id: want, view: null, raw: "" };
    this.recheck();
    this.showStatus();
    this.checkQuiet();
    if (want === null) return this.host.emit("tour", { id: null, view: null });
    void this.tell(want, true).then(() => this.refresh(want));
  }

  /** The 2-second re-read is on or off by whether a tour is open and `.vnoc/` is above the opened folder; asked again when the git root becomes known. */
  recheck(): void {
    const want = shouldPoll(this.open !== null, this.host.vnocAbove());
    if (want && !this.poll) this.poll = this.host.every(REREAD_MS, () => (this.open ? void this.refresh(this.open.id) : undefined));
    if (!want && this.poll) {
      this.poll.stop();
      this.poll = undefined;
    }
  }

  /** The list, asked once at the start so that tours that appear later are known as new (and `toursChanged` always has a list to ask again for). Nothing is announced for what is there. */
  start(): void {
    void this.list().catch((e) => this.host.log(`vnoc/tours: ${String((e as Error).message ?? e)}`));
  }

  /** The core started again: it does not know a window shows a tour. Told again, and the tour asked for again (the core's answer may differ: the files were read anew). */
  restarted(): void {
    const open = this.open;
    if (!open) return;
    void this.tell(open.id, true).then(() => this.refresh(open.id));
  }

  /** `vnoc/toursChanged`. */
  toursChanged(ids: readonly TourId[]): void {
    const ask = whatToAsk(ids, this.open?.id ?? null, this.rows !== null);
    if (ask.tour && this.open) void this.refresh(this.open.id);
    if (ask.list) void this.list();
  }

  /** `vnoc/tourPlaceChanged`: the place of a tour moved from outside. Taken for the open tour; a tour that is not on screen is not opened. */
  placeChanged(id: TourId, place: Place): void {
    if (this.open?.id === id) this.setPlace(place);
  }

  /**
   * The page's `tours`: the core's list. The plain list is held, and sent again when the core says a tour changed. It is asked with `check`: the ids of the rows held, so their
   * `words` say `changed` when the core's check says so (rung 1 or 2 answers: nothing is read for a tour no file of which changed). The first list in a window holds no rows and
   * asks twice: plain (sent at once), then with the ids it answered (the same, with the check). A tour the list did not have yet is checked at the next list. With `on` (a
   * structure key) it is the tours on that node, and nothing is held.
   */
  async list(params?: { on?: string; check?: TourId[] }): Promise<TourRow[]> {
    if (params?.on !== undefined) return (await this.host.request("vnoc/tours", { on: params.on, ...(params.check ? { check: params.check } : {}) })).tours;
    const checked = new Set(params?.check ?? readableIds(this.rows ?? []));
    const ask = async (ids: Set<TourId>) => (await this.host.request("vnoc/tours", ids.size > 0 ? { check: [...ids] } : undefined)).tours;
    let tours = await ask(checked);
    const unchecked = readableIds(tours).filter((id) => !checked.has(id));
    if (unchecked.length > 0) {
      this.publish(tours);
      tours = await ask(new Set(readableIds(tours)));
    }
    this.publish(tours);
    return tours;
  }

  private publish(tours: TourRow[]): void {
    this.rows = tours;
    this.notice(tours);
    this.host.emit("tours", { tours });
  }

  /** The page's `tourCode`: one stop as it was written (`vnoc/tourCode`), `null` when the core has none to give. */
  async code(id: TourId, stop: StopId): Promise<AsWritten | null> {
    return this.host.request("vnoc/tourCode", { id, stop });
  }

  /** The page's `tourCalls`: the calls of the tour's written stops that are not on it (`vnoc/tourCalls`). */
  async calls(id: TourId): Promise<TourCallsAnswer> {
    return this.host.request("vnoc/tourCalls", { id });
  }

  /**
   * A tour that was not in the last list and is being written, once (W11): with the canvas visible and no tour open it becomes the window's tour context, at the cover; otherwise it is
   * announced, whatever the canvas shows. Nothing already open is replaced.
   */
  private notice(tours: TourRow[]): void {
    const fresh = toAnnounce(tours, this.known, this.announced);
    this.known = new Set(tours.map((r) => r.id));
    let tourOpen = tourContext(this.host.session()) !== undefined;
    for (const row of fresh) {
      this.announced.add(row.id);
      const what = appearance({ visible: this.host.canvasVisible(), tourOpen });
      if (what === "open") {
        tourOpen = true; // the next one of this list finds a tour open
        this.host.open(row);
      } else if (what === "notify") this.host.announce(row);
    }
  }

  /** The status bar item: `▶ Tour 3/10` while a tour is open (the agent's words after it while it writes), nothing when none is. Told only when the text changed. */
  private showStatus(): void {
    const open = this.open;
    const view = open?.view && "stops" in open.view ? open.view : null;
    const text = open === null ? null : view === null ? "▶ Tour" : statusBarText(view);
    if (text === this.status) return;
    this.status = text;
    this.host.statusBar(text);
  }

  /** The open tour is asked for again every 30 s while it is being written (see `QUIET_CHECK_MS`); the timer stops when it is finished or stopped, or closed. */
  private checkQuiet(): void {
    const view = this.open?.view;
    const want = shouldCheckQuiet(view && "stops" in view ? view.state : null);
    if (want && !this.quietCheck) this.quietCheck = this.host.every(QUIET_CHECK_MS, () => (this.open ? void this.refresh(this.open.id) : undefined));
    if (!want && this.quietCheck) {
      this.quietCheck.stop();
      this.quietCheck = undefined;
    }
  }

  /** ■ Stop: the core writes the halt, and the tour is asked for again, so the state says `stopping` at once. What the agent does with it is the core's (`stopped` comes when it answers). */
  async halt(id: TourId): Promise<TourHaltAnswer> {
    const answer = await this.host.request("vnoc/tourHalt", { id });
    if (this.open?.id === id) await this.refresh(id);
    return answer;
  }

  /** The page's `tour`: what the host holds when that is the open tour, else the core's answer. */
  async get(id: TourId): Promise<Answer> {
    if (this.open?.id === id && this.open.view) return this.open.view;
    return this.host.request("vnoc/tour", { id });
  }

  /** The page's `tourGo`: the core makes the move; the place it answers is taken as the place. */
  async go(id: TourId, move: TourMove): Promise<Place> {
    const place = await this.host.request("vnoc/tourGo", { id, move });
    if (this.open?.id === id) this.setPlace(place);
    return place;
  }

  /** The open tour as the pages were last sent it, for the test suite: `null` when none is open or nothing was answered yet. */
  heldTour(): Answer | null {
    return this.open?.view ?? null;
  }

  /** The window is going: the core is told the tour is not shown any more. */
  dispose(): void {
    this.poll?.stop();
    this.poll = undefined;
    this.quietCheck?.stop();
    this.quietCheck = undefined;
    this.host.statusBar(null);
    if (this.open) void this.tell(this.open.id, false);
    this.open = null;
  }

  // ── inside ──

  private async tell(id: TourId, open: boolean): Promise<void> {
    try {
      await this.host.request("vnoc/tourOpen", { id, open });
    } catch (e) {
      this.host.log(`vnoc/tourOpen ${id} ${open}: ${String((e as Error).message ?? e)}`);
    }
  }

  /** Asks the core for the tour and takes the answer, when the tour is still the open one. A tour the core cannot find is shown as unreadable, with its words. */
  private async refresh(id: TourId): Promise<void> {
    let answer: Answer;
    try {
      answer = await this.host.request("vnoc/tour", { id });
    } catch (e) {
      answer = { unreadable: String((e as Error).message ?? e) };
    }
    if (this.open?.id === id) this.take(answer);
  }

  private setPlace(place: Place): void {
    const view = this.open?.view;
    if (view && "stops" in view) this.take({ ...view, place });
  }

  /** The answer is what the pages draw. One that is the same text as the last (a re-read that found nothing) sends nothing. After it, the selection follows the place. */
  private take(answer: Answer): void {
    if (!this.open) return;
    const raw = JSON.stringify(answer);
    if (raw === this.open.raw) return;
    this.open = { id: this.open.id, view: answer, raw };
    this.select(answer);
    this.checkQuiet();
    this.showStatus();
    this.host.emit("tour", { id: this.open.id, view: answer });
  }

  /** The tour context's selection is the current stop's node: the ring, the heavy edges and the docs panel follow it as they follow any selection. */
  private select(answer: Answer): void {
    const context = tourContext(this.host.session());
    if (context && "stops" in answer) this.host.dispatch({ type: "tourSelect", key: selectionKey(answer) }, context.id);
  }
}
