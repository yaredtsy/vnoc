import assert from "node:assert/strict";
import { test } from "node:test";

import type { TreeNode } from "@vnoc/shared/messages";
import { type Intent, MAIN, type Session, emptySession, reduce, tourContextId } from "@vnoc/shared/session.ts";
import type { Place, TourRow, TourStop, TourView } from "@vnoc/shared/tours.ts";
import { QUIET_CHECK_MS, REREAD_MS, Tours, type ToursDeps, pickItem, selectionKey, shouldCheckQuiet, shouldPoll, tourContext, vnocIsAbove, whatToAsk } from "./tours.ts";

// The decisions of the host's holder that need no VS Code: which notification asks again, when the 2-second re-read runs, what the selection is for a place.

const node = (id: string, kind: TreeNode["kind"], children: TreeNode[] = []): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
const tree = node("/", "folder", [node("a.py", "file", [node("a.py::f", "function"), node("a.py::g", "function")]), node("b.py", "file")]);
const calls = () => undefined;

const stop = (id: string, key: string | null, steps: string[]): TourStop => ({
  number: Number(id.slice(1)), id, depth: 0, planned: key === null, kind: key === null ? null : "function", name: id, path: "a.py", qualname: id, key, steps: steps.length, blocks: [],
  reading: steps.map((step) => ({ id: step, kind: "intro" as const, depth: step.includes(".") ? 1 : 0, parent: null, title: null, text: "", lines: null })),
});
const place = (at: string, over: Partial<Place> = {}): Place => ({
  at, seen: [], back: [], moved: "", by: "reader", open: true,
  // what the core makes of `at` (the tests do not compute it: `s1.b1` is stop s1)
  stop: at === "cover" || at === "end" ? null : at.split(".")[0], position: null, unseen: 0, first_unseen: null, ...over,
});
const view = (at: string, over: Partial<TourView> = {}): TourView => ({
  id: "t-1", title: "T", detail: "normal", state: "finished", folder: "private", written: 3, planned: 0, steps: 5, minutes: 1, pinned: false, writing: null, offers: null, problems: [],
  place: place(at),
  stops: [stop("s1", "a.py::f", ["s1", "s1.b1"]), stop("s2", "a.py::g", ["s2"]), stop("s3", null, ["s3"])],
  ...over,
});

test("a change to a tour asks again for what the window shows: the open tour when it is named, the list when one is held", () => {
  assert.deepEqual(whatToAsk(["t-1"], "t-1", false), { tour: true, list: false });
  assert.deepEqual(whatToAsk(["t-2"], "t-1", false), { tour: false, list: false }, "another tour, and no list held");
  assert.deepEqual(whatToAsk(["t-2"], "t-1", true), { tour: false, list: true }, "any tour may be a row of the list");
  assert.deepEqual(whatToAsk(["t-2", "t-1"], "t-1", true), { tour: true, list: true });
  assert.deepEqual(whatToAsk(["t-1"], null, false), { tour: false, list: false }, "no tour open");
  assert.deepEqual(whatToAsk([], "t-1", false), { tour: false, list: false });
});

test(".vnoc/ is above the opened folder when the git root is a folder above it; unknown, or the same folder, or elsewhere, is not", () => {
  assert.equal(vnocIsAbove("/work/repo/app", "/work/repo"), true);
  assert.equal(vnocIsAbove("/work/repo/app/deeper", "/work/repo"), true);
  assert.equal(vnocIsAbove("/work/repo", "/work/repo"), false, "the watcher sees .vnoc/ in the folder that was opened");
  assert.equal(vnocIsAbove("/work/repo", "/work/repo/"), false);
  assert.equal(vnocIsAbove("/work/other", "/work/repo"), false);
  assert.equal(vnocIsAbove("/work/repo", "/work/repo/app"), false, "a git root below the opened folder is below it");
  assert.equal(vnocIsAbove("/work/repo", undefined), false, "the core has not said where the git root is");
  assert.equal(vnocIsAbove(undefined, "/work/repo"), false, "no folder is open");
});

test("the re-read runs only while a tour is open and .vnoc/ is above the opened folder", () => {
  assert.equal(shouldPoll(true, true), true);
  assert.equal(shouldPoll(true, false), false);
  assert.equal(shouldPoll(false, true), false);
  assert.equal(shouldPoll(false, false), false);
});

test("the selection is the current stop's key, read from `place.stop`; at the cover or the end, the first stop's; none for a stop with no node", () => {
  assert.equal(selectionKey(view("cover")), "a.py::f");
  assert.equal(selectionKey(view("end")), "a.py::f");
  assert.equal(selectionKey(view("s1.b1")), "a.py::f");
  assert.equal(selectionKey(view("s2")), "a.py::g");
  assert.equal(selectionKey(view("s3")), null, "a planned stop has no node");
  assert.equal(selectionKey(view("s1", { place: place("s1", { stop: "s2" }) })), "a.py::g", "the core's word is the stop, not the step id");
  assert.equal(selectionKey(view("s9")), null, "a stop the view does not have selects nothing");
  assert.equal(selectionKey(view("cover", { stops: [] })), null, "a tour with no stop");
});

test("a row of Open Tour… says what the row says", () => {
  const row: TourRow = { id: "t-1", folder: "private", readable: true, title: "Retry charge", question: "Why does it retry?", quiet: false, root: null, place: null, offers: null, words: "stop 4 · 9 stops" };
  assert.deepEqual({ ...pickItem(row), row: undefined }, { label: "Retry charge", description: "stop 4 · 9 stops", detail: "Why does it retry?", row: undefined });
  const broken: TourRow = { id: "t-9", folder: "private", readable: false, problem: "line 1: not a tour", quiet: false, root: null, place: null, offers: null, words: "cannot be read" };
  assert.deepEqual({ ...pickItem(broken), row: undefined }, { label: "t-9", description: "cannot be read", detail: "line 1: not a tour", row: undefined });
});

// ── the holder, with a host that is only a record ─────────────────────────────────────────────────────────────────────────

/** A host: the core answers from `answers`, every request and event is kept, the session is a real one, and `every` is a clock the test turns. */
function makeHost(opts: { above: boolean; answer?: TourView; rows?: TourRow[]; canvasShows?: boolean }) {
  let session: Session = emptySession();
  const requests: [string, unknown][] = [];
  const events: [string, unknown][] = [];
  const dispatched: [Intent, string][] = [];
  const timers: { ms: number; run: () => void; stopped: boolean }[] = [];
  let answer = opts.answer ?? view("cover");
  let rows = opts.rows ?? [];
  let canvasShows = opts.canvasShows ?? false;
  const status: (string | null)[] = [];
  const announced: string[] = [];
  const host: ToursDeps = {
    request: (async (method: string, params: unknown) => {
      requests.push([method, params]);
      if (method === "vnoc/tour") return answer;
      if (method === "vnoc/tours") return { tours: rows };
      return place("cover");
    }) as ToursDeps["request"],
    session: () => session,
    dispatch: (intent, context) => {
      dispatched.push([intent, context]);
      session = reduce(session, context, intent, tree, calls);
    },
    emit: ((name: string, data: unknown) => void events.push([name, data])) as ToursDeps["emit"],
    vnocAbove: () => opts.above,
    every: (ms, run) => {
      const timer = { ms, run, stopped: false };
      timers.push(timer);
      return { stop: () => void (timer.stopped = true) };
    },
    log: () => {},
    statusBar: (text) => void status.push(text),
    announce: (row) => void announced.push(row.id),
    canvasShowsTour: () => canvasShows,
  };
  return {
    status,
    announced,
    setRows: (r: TourRow[]) => (rows = r),
    setCanvasShows: (shows: boolean) => (canvasShows = shows),
    host,
    requests,
    events,
    dispatched,
    timers,
    setAnswer: (v: TourView) => (answer = v),
    session: () => session,
    openTour: (id = "t-1") => void (session = reduce(session, MAIN, { type: "tourOpen", id }, tree, calls)),
    closeTour: (id = "t-1") => void (session = reduce(session, MAIN, { type: "closeContext", id: tourContextId(id) }, tree, calls)),
    asked: (method: string) => requests.filter(([m]) => m === method).length,
  };
}
const settle = () => new Promise((resolve) => setImmediate(resolve));

test("a tour that becomes the tour context is told to the core as open, asked for, and its stop is selected; closing tells the core it is not shown", async () => {
  const h = makeHost({ above: false });
  const tours = new Tours(h.host);
  h.openTour();
  tours.sync();
  await settle();
  assert.deepEqual(h.requests, [["vnoc/tourOpen", { id: "t-1", open: true }], ["vnoc/tour", { id: "t-1" }]]);
  assert.equal(h.session().contexts[tourContextId("t-1")].selected, "a.py::f", "the cover selects the first stop");
  assert.deepEqual(h.events.map(([name]) => name), ["tour"]);
  assert.equal((h.events[0][1] as { id: string }).id, "t-1");
  tours.sync(); // nothing changed: nothing is asked
  assert.equal(h.requests.length, 2);
  h.closeTour();
  tours.sync();
  await settle();
  assert.deepEqual(h.requests.slice(2), [["vnoc/tourOpen", { id: "t-1", open: false }]]);
  assert.deepEqual(h.events.at(-1), ["tour", { id: null, view: null }]);
  assert.equal(tourContext(h.session()), undefined);
});

test("a second tour tells the core the first is not shown and the second is", async () => {
  const h = makeHost({ above: false });
  const tours = new Tours(h.host);
  h.openTour("t-1");
  tours.sync();
  await settle();
  h.requests.length = 0;
  h.openTour("t-2");
  tours.sync();
  await settle();
  assert.deepEqual(h.requests, [["vnoc/tourOpen", { id: "t-1", open: false }], ["vnoc/tourOpen", { id: "t-2", open: true }], ["vnoc/tour", { id: "t-2" }]]);
});

test("a notice that names the open tour asks for it again; one that names another asks nothing, or the list when one is held", async () => {
  const h = makeHost({ above: false });
  const tours = new Tours(h.host);
  h.openTour();
  tours.sync();
  await settle();
  const base = h.asked("vnoc/tour");
  tours.toursChanged(["t-2"]);
  await settle();
  assert.deepEqual([h.asked("vnoc/tour"), h.asked("vnoc/tours")], [base, 0]);
  tours.toursChanged(["t-1"]);
  await settle();
  assert.deepEqual([h.asked("vnoc/tour"), h.asked("vnoc/tours")], [base + 1, 0]);
  await tours.list(); // a page listed: the list is held
  tours.toursChanged(["t-2"]);
  await settle();
  assert.deepEqual([h.asked("vnoc/tour"), h.asked("vnoc/tours")], [base + 1, 2], "the list is asked again, the tour is not");
  assert.deepEqual(h.events.filter(([name]) => name === "tours").length, 2);
});

test("a place from outside is taken for the open tour and moves the selection; the place of a tour that is not open is ignored", async () => {
  const h = makeHost({ above: false });
  const tours = new Tours(h.host);
  h.openTour();
  tours.sync();
  await settle();
  const before = h.events.length;
  tours.placeChanged("t-2", place("s2"));
  assert.equal(h.events.length, before, "a tour that is not on screen is not opened");
  tours.placeChanged("t-1", place("s2"));
  assert.equal(h.session().contexts[tourContextId("t-1")].selected, "a.py::g");
  const last = h.events.at(-1)!;
  assert.equal(last[0], "tour");
  assert.equal(((last[1] as { view: TourView }).view).place.at, "s2", "the pages are sent the view with the new place");
  assert.equal(h.asked("vnoc/tour"), 1, "the place came with the notice: nothing is asked");
  // a place on a stop with no node leaves nothing selected
  tours.placeChanged("t-1", place("s3"));
  assert.equal(h.session().contexts[tourContextId("t-1")].selected, null);
});

test("a page's move goes to the core, and the place it answers is taken as the place", async () => {
  const h = makeHost({ above: false });
  const tours = new Tours(h.host);
  h.openTour();
  tours.sync();
  await settle();
  const answerPlace = place("s2");
  (h.host as { request: unknown }).request = async (method: string, params: unknown) => {
    h.requests.push([method, params]);
    return answerPlace;
  };
  const got = await tours.go("t-1", "next-stop");
  assert.deepEqual(h.requests.at(-1), ["vnoc/tourGo", { id: "t-1", move: "next-stop" }]);
  assert.equal(got, answerPlace);
  assert.equal(h.session().contexts[tourContextId("t-1")].selected, "a.py::g");
  assert.equal(((await tours.get("t-1")) as TourView).place.at, "s2", "the host holds the open tour with its new place");
});

test("the 2-second re-read runs only when .vnoc/ is above the opened folder, and only while a tour is open", async () => {
  // .vnoc/ in the opened folder: the watcher sees it; no timer, ever
  const near = makeHost({ above: false });
  const nearTours = new Tours(near.host);
  near.openTour();
  nearTours.sync();
  await settle();
  nearTours.recheck();
  assert.equal(near.timers.length, 0);

  // .vnoc/ above it: a timer of 2 seconds while the tour is open, which asks `vnoc/tour` again
  const far = makeHost({ above: true });
  const farTours = new Tours(far.host);
  farTours.recheck();
  assert.equal(far.timers.length, 0, "no tour open: nothing to re-read");
  far.openTour();
  farTours.sync();
  await settle();
  assert.deepEqual(far.timers.map((t) => t.ms), [REREAD_MS]);
  assert.equal(REREAD_MS, 2000);
  const asked = far.asked("vnoc/tour");
  far.timers[0].run();
  await settle();
  assert.equal(far.asked("vnoc/tour"), asked + 1);
  // the answer is the same text: the pages are sent nothing, and nothing is dispatched again
  const events = far.events.length;
  far.timers[0].run();
  await settle();
  assert.equal(far.events.length, events);
  // a place moved by `vnoc tour go` in another process shows in the next read
  far.setAnswer(view("s2"));
  far.timers[0].run();
  await settle();
  assert.equal(far.session().contexts[tourContextId("t-1")].selected, "a.py::g");
  // the context closes: the timer is stopped, and starting `recheck` again starts none
  far.closeTour();
  farTours.sync();
  assert.equal(far.timers[0].stopped, true);
  farTours.recheck();
  assert.equal(far.timers.length, 1);
});

test("the git root becoming known starts the re-read of a tour that was opened before it", async () => {
  let above = false;
  const h = makeHost({ above: false });
  h.host.vnocAbove = () => above;
  const tours = new Tours(h.host);
  h.openTour();
  tours.sync();
  await settle();
  assert.equal(h.timers.length, 0);
  above = true;
  tours.recheck();
  assert.equal(h.timers.length, 1);
  tours.recheck();
  assert.equal(h.timers.length, 1, "once");
});

test("a core that started again is told the tour is shown, and asked for it again", async () => {
  const h = makeHost({ above: false });
  const tours = new Tours(h.host);
  tours.restarted(); // nothing open: nothing to say
  assert.equal(h.requests.length, 0);
  h.openTour();
  tours.sync();
  await settle();
  h.requests.length = 0;
  tours.restarted();
  await settle();
  assert.deepEqual(h.requests, [["vnoc/tourOpen", { id: "t-1", open: true }], ["vnoc/tour", { id: "t-1" }]]);
});

test("the window going tells the core the tour is not shown and stops the re-read", async () => {
  const h = makeHost({ above: true });
  const tours = new Tours(h.host);
  h.openTour();
  tours.sync();
  await settle();
  h.requests.length = 0;
  tours.dispose();
  assert.deepEqual(h.requests, [["vnoc/tourOpen", { id: "t-1", open: false }]]);
  assert.equal(h.timers[0].stopped, true);
});

test("a tour the core cannot find is held as unreadable, with the core's words, and nothing is selected", async () => {
  const h = makeHost({ above: false });
  (h.host as { request: unknown }).request = async (method: string) => {
    if (method === "vnoc/tour") throw new Error("no tour t-1");
    return place("cover");
  };
  const tours = new Tours(h.host);
  h.openTour();
  tours.sync();
  await settle();
  assert.deepEqual(h.events.at(-1), ["tour", { id: "t-1", view: { unreadable: "no tour t-1" } }]);
  assert.equal(h.session().contexts[tourContextId("t-1")].selected, null);
});

// ── the tour read while it is written (docs/26-tours/05-on-screen.md §3, design 04 §6) ───────────────────────────────────

const at = (at: string, stopNo: number, of: number) => place(at, { position: { stop: stopNo, of, step: 1, steps: 1 } });
const row = (id: string, over: Partial<TourRow> = {}): TourRow => ({ id, folder: "private", readable: true, title: `Tour ${id}`, state: "writing", quiet: false, root: null, place: null, offers: null, words: "", ...over });

test("the status bar item says `▶ Tour 3/10` from the place the core gave while a tour is open, follows a move, and goes when the tour closes", async () => {
  const h = makeHost({ above: false, answer: view("s2", { place: at("s2", 2, 3) }) });
  const tours = new Tours(h.host);
  assert.deepEqual(h.status, [], "no tour is open: nothing is said");
  h.openTour();
  tours.sync();
  await settle();
  assert.deepEqual(h.status, ["▶ Tour", "▶ Tour 2/3"], "open, then the place the core answered");
  tours.placeChanged("t-1", at("s3", 3, 3));
  assert.equal(h.status.at(-1), "▶ Tour 3/3");
  tours.placeChanged("t-1", at("s3", 3, 3));
  assert.equal(h.status.length, 3, "the same text is not said again");
  h.closeTour();
  tours.sync();
  assert.equal(h.status.at(-1), null, "no tour open: the item is hidden");
});

test("the status bar item says what the agent is doing while it writes, and `no news` from the row that is the tour's", async () => {
  const written = view("s2", { state: "writing", written: 2, planned: 8, writing: "s3", place: at("s2", 2, 10) });
  const h = makeHost({ above: false, answer: written, rows: [row("t-1")] });
  const tours = new Tours(h.host);
  await tours.list();
  h.openTour();
  tours.sync();
  await settle();
  assert.equal(h.status.at(-1), "▶ Tour 2/10 · writing 3/10");
  h.setRows([row("t-1", { quiet: true })]);
  await tours.list();
  assert.equal(h.status.at(-1), "▶ Tour 2/10 · no news", "the list said the file was quiet");
});

test("a tour that appears while the window is open is announced once, and only when the canvas is not showing a tour; nothing opens by itself", async () => {
  const h = makeHost({ above: false, rows: [row("t-0", { state: "finished" })] });
  const tours = new Tours(h.host);
  tours.start();
  await settle();
  assert.deepEqual(h.announced, [], "the tours that were there are not announced");
  h.setRows([row("t-1"), row("t-0", { state: "finished" })]);
  tours.toursChanged(["t-1"]);
  await settle();
  assert.deepEqual(h.announced, ["t-1"]);
  tours.toursChanged(["t-1"]); // the agent adds a stop: the file changes again
  await settle();
  assert.deepEqual(h.announced, ["t-1"], "once per tour");
  assert.equal(tourContext(h.session()), undefined, "nothing was opened");
  // a canvas that is showing a tour is not told: the list has the new one, and the open tour stays
  h.setCanvasShows(true);
  h.setRows([row("t-2"), row("t-1"), row("t-0", { state: "finished" })]);
  tours.toursChanged(["t-2"]);
  await settle();
  assert.deepEqual(h.announced, ["t-1"]);
  h.setCanvasShows(false);
  tours.toursChanged(["t-2"]);
  await settle();
  assert.deepEqual(h.announced, ["t-1"], "t-2 was seen while a tour was shown: that was its one chance");
});

test("the list is asked again every 30 seconds only while the open tour is being written", async () => {
  assert.equal(QUIET_CHECK_MS, 30_000);
  assert.deepEqual(["writing", "stopping", "stopped", "finished"].map((s) => shouldCheckQuiet(s as TourView["state"])), [true, true, false, false]);
  assert.equal(shouldCheckQuiet(null), false);
  const h = makeHost({ above: false, answer: view("s1", { state: "writing", writing: "s3" }), rows: [row("t-1")] });
  const tours = new Tours(h.host);
  h.openTour();
  tours.sync();
  await settle();
  assert.deepEqual(h.timers.map((t) => t.ms), [QUIET_CHECK_MS]);
  const listed = h.asked("vnoc/tours");
  h.timers[0].run();
  await settle();
  assert.equal(h.asked("vnoc/tours"), listed + 1, "a quiet tour changes no file: the list says it");
  // the tour is finished: nothing to wait for
  h.setAnswer(view("s1", { state: "finished" }));
  tours.toursChanged(["t-1"]);
  await settle();
  assert.equal(h.timers[0].stopped, true);
});

test("■ Stop is the core's halt, and the tour is asked for again so that the state says stopping", async () => {
  const h = makeHost({ above: false, answer: view("s1", { state: "writing", writing: "s3" }) });
  const tours = new Tours(h.host);
  h.openTour();
  tours.sync();
  await settle();
  h.setAnswer(view("s1", { state: "stopping" }));
  await tours.halt("t-1");
  assert.deepEqual(h.requests.filter(([m]) => m === "vnoc/tourHalt"), [["vnoc/tourHalt", { id: "t-1" }]]);
  assert.equal(((await tours.get("t-1")) as TourView).state, "stopping", "the pages are sent the state the core has now");
});
