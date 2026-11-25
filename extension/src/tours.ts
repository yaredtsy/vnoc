import * as path from "path";

import type { Events } from "@vnoc/shared/messages";
import type { ContextId, ContextState, Intent, Session } from "@vnoc/shared/session.ts";
import type { Place, TourCoreRequests, TourId, TourMove, TourRow, TourUnreadable, TourView } from "@vnoc/shared/tours.ts";

// The host's holder of tours (docs/26-tours/05-on-screen.md §5, §6). It asks the core, holds the open tour's `TourView` with its place, passes it to the pages, and sets
// the tour context's selection from the place. It computes nothing about a tour (P1): which stop is current, what comes next, whether anything changed are the core's
// answers. No `vscode` in this file: what the host gives it is in `ToursDeps`, so the decisions can be tested with `node --test`.

/** How often the open tour is asked for again when `.vnoc/` is above the opened folder (open check 11): the core answers from two files' lengths and times. */
export const REREAD_MS = 2000;

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

/**
 * The key of the node the tour context selects for a place the host holds: the current stop's, found as the stop whose steps include the place's step; at the cover or
 * at the end, the first stop's. `null` when that stop has no node in the tree (planned, or gone).
 */
export function selectionKey(view: TourView): string | null {
  const at = view.place.at;
  const first = view.stops[0];
  const stop = at === "cover" || at === "end" ? first : (view.stops.find((s) => s.reading.some((step) => step.id === at)) ?? first);
  return stop?.key ?? null;
}

/**
 * One row of Open Tour…'s QuickPick (design 02 frame 10), in the core's own words and in its order: the label is the tour's title (its id for a tour that cannot be read),
 * the description how the tour is on a node when the row says it, the detail the question the tour answers, or why it cannot be read. The core has no line 2 for a row
 * in `TourRow` yet (its `list` text has one: stops, when, where you are); until it does, the picker says no more than the row does.
 */
export function pickItem(row: TourRow): { label: string; description?: string; detail?: string; row: TourRow } {
  return { label: row.title ?? row.id, description: row.on?.words, detail: row.readable ? row.question : row.problem, row };
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
  private poll: { stop(): void } | undefined;

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

  /** The page's `tours`: the core's list. The plain list is held, and sent again when the core says a tour changed. */
  async list(on?: string): Promise<TourRow[]> {
    const { tours } = await this.host.request("vnoc/tours", on === undefined ? undefined : { on });
    if (on !== undefined) return tours;
    this.rows = tours;
    this.host.emit("tours", { tours });
    return tours;
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
    this.host.emit("tour", { id: this.open.id, view: answer });
  }

  /** The tour context's selection is the current stop's node: the ring, the heavy edges and the docs panel follow it as they follow any selection. */
  private select(answer: Answer): void {
    const context = tourContext(this.host.session());
    if (context && "stops" in answer) this.host.dispatch({ type: "tourSelect", key: selectionKey(answer) }, context.id);
  }
}
