import assert from "node:assert/strict";
import { test } from "node:test";

import type { Wanted } from "./calls.ts";
import { type Clock, CallScheduler } from "./callScheduler.ts";
import type { CallTreeResult } from "./messages";

// ── fakes: a clock that moves when told, and a request that answers when told ─────────────────────────────────────────

class FakeClock implements Clock {
  now = 0;
  private timers = new Map<number, { at: number; fn: () => void }>();
  private next = 0;
  setTimeout(fn: () => void, ms: number) {
    this.timers.set(++this.next, { at: this.now + ms, fn });
    return this.next;
  }
  clearTimeout(handle: unknown) {
    this.timers.delete(handle as number);
  }
  advance(ms: number) {
    const end = this.now + ms;
    for (;;) {
      const due = [...this.timers.entries()].filter(([, t]) => t.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      this.timers.delete(due[0]);
      this.now = due[1].at;
      due[1].fn();
    }
    this.now = end;
  }
  get pending() {
    return this.timers.size;
  }
}

const tree = (revision: number): CallTreeResult => ({ revision, root: { key: null, name: "x", qualname: "x", path: null, kind: "function", call_count: 1, call_sites: [], marks: [], children: [] } });

interface Request {
  key: string;
  signal: AbortSignal;
  resolve(result: CallTreeResult): void;
  reject(e: Error): void;
}

function setup(options: { concurrency?: number; keep?: number } = {}) {
  const clock = new FakeClock();
  const requests: Request[] = [];
  const log: string[] = [];
  const scheduler = new CallScheduler({
    ...options,
    clock,
    fetch: (key, signal) => new Promise((resolve, reject) => requests.push({ key, signal, resolve, reject })),
    onTree: (key, r) => log.push(`tree ${key}@${r.revision}`),
    onState: (key, state, message) => log.push(`${state} ${key}${message ? `: ${message}` : ""}`),
    onDropped: (key) => log.push(`dropped ${key}`),
    onReset: () => log.push("reset"),
  });
  const started = () => requests.map((r) => r.key);
  /** lets the promise callbacks run */
  const settle = () => new Promise((resolve) => setImmediate(resolve));
  return { clock, requests, log, scheduler, started, settle };
}
const open = (...keys: string[]): Wanted[] => keys.map((key) => ({ key, reason: "open" as const }));
const prefetch = (key: string): Wanted => ({ key, reason: "prefetch" });

test("two requests at a time; the prefetch waits for what is open, whatever the order it was listed in", async () => {
  const { requests, scheduler, started, settle } = setup();
  scheduler.setWanted([prefetch("p"), ...open("a", "b", "c")]);
  assert.deepEqual(started(), ["a", "b"], "two at once, and not the prefetch");
  requests[0].resolve(tree(1));
  await settle();
  assert.deepEqual(started(), ["a", "b", "c"], "a free place goes to what is open");
  requests[1].resolve(tree(1));
  await settle();
  assert.deepEqual(started(), ["a", "b", "c", "p"], "the prefetch last");
});

test("nothing is asked twice: a request already out, a tree held, the same list again", async () => {
  const { requests, scheduler, started, settle } = setup();
  scheduler.setWanted(open("a"));
  scheduler.setWanted(open("a"));
  assert.deepEqual(started(), ["a"], "one request, not two");
  requests[0].resolve(tree(1));
  await settle();
  scheduler.setWanted(open("a"));
  assert.deepEqual(started(), ["a"], "its tree is held and fresh");
});

test("a request nothing wants any more is cancelled: its signal fires, and its late answer is dropped", async () => {
  const { requests, scheduler, log, settle } = setup();
  scheduler.setWanted(open("a"));
  assert.equal(requests[0].signal.aborted, false);
  scheduler.setWanted([]);
  assert.equal(requests[0].signal.aborted, true, "the signal fired");
  requests[0].resolve(tree(1)); // too late
  await settle();
  assert.equal(scheduler.tree("a"), undefined, "no tree is kept for it");
  assert.deepEqual(log, ["loading a", "idle a"], "loading, then idle (cancelled); never a tree");
});

test("answers out of order: only the newest request's answer for a key is kept", async () => {
  const { requests, scheduler, log, settle } = setup();
  scheduler.setWanted(open("a")); // request 1
  scheduler.setWanted([]); // cancelled
  scheduler.setWanted(open("a")); // request 2
  assert.equal(requests.length, 2);
  requests[1].resolve(tree(2));
  await settle();
  requests[0].resolve(tree(1)); // the older one arrives last
  await settle();
  assert.equal(scheduler.tree("a")?.revision, 2, "the newest request's answer stays");
  assert.deepEqual(log.filter((l) => l.startsWith("tree")), ["tree a@2"], "the other one never spoke");
});

test("a failure is state failed with the message, it is not asked again by itself, and a retry fetches again", async () => {
  const { requests, scheduler, log, settle } = setup();
  scheduler.setWanted(open("a"));
  requests[0].reject(new Error("the driver crashed"));
  await settle();
  assert.equal(scheduler.stateOf("a"), "failed");
  assert.deepEqual(log, ["loading a", "failed a: the driver crashed"]);
  scheduler.setWanted(open("a")); // the same list again: no loop of failing requests
  assert.equal(requests.length, 1);
  scheduler.retry("a");
  assert.equal(requests.length, 2, "retry asks again");
  requests[1].resolve(tree(1));
  await settle();
  assert.deepEqual([scheduler.stateOf("a"), scheduler.tree("a")?.revision], ["idle", 1]);
});

test("a key that stops being wanted forgets its failure: wanting it again asks again", async () => {
  const { requests, scheduler, settle } = setup();
  scheduler.setWanted(open("a"));
  requests[0].reject(new Error("boom"));
  await settle();
  scheduler.setWanted([]);
  scheduler.setWanted(open("a"));
  assert.equal(requests.length, 2);
});

test("a stale tree that is wanted is fetched again 800 ms after the LAST index change: three changes in 500 ms are one fetch", async () => {
  const { requests, scheduler, clock, log, settle } = setup();
  scheduler.setWanted(open("a"));
  requests[0].resolve(tree(1));
  await settle();
  scheduler.indexChanged(2); // t = 0
  clock.advance(200);
  scheduler.indexChanged(3); // t = 200
  clock.advance(300);
  scheduler.indexChanged(4); // t = 500
  clock.advance(799); // t = 1299: 799 ms after the last
  assert.equal(requests.length, 1, "not yet");
  clock.advance(1); // t = 1300
  assert.equal(requests.length, 2, "one fetch, 800 ms after the last change");
  assert.equal(scheduler.tree("a")?.revision, 1, "the old tree stays in place while the new one is made");
  assert.equal(scheduler.stateOf("a"), "loading");
  requests[1].resolve(tree(4));
  await settle();
  assert.equal(scheduler.tree("a")?.revision, 4);
  assert.deepEqual(log.slice(-2), ["tree a@4", "idle a"]);
  clock.advance(5000);
  assert.equal(requests.length, 2, "and nothing more");
});

test("a stale tree that nothing wants is dropped, not fetched", async () => {
  const { requests, scheduler, clock, log, settle } = setup();
  scheduler.setWanted(open("a", "b"));
  requests[0].resolve(tree(1));
  requests[1].resolve(tree(1));
  await settle();
  scheduler.setWanted(open("b")); // a is unwanted but kept
  scheduler.indexChanged(2);
  clock.advance(800);
  assert.equal(scheduler.tree("a"), undefined, "dropped");
  assert.ok(log.includes("dropped a"));
  assert.equal(requests.length, 3, "b is fetched again");
  assert.equal(requests[2].key, "b");
});

test("a fresh tree is not fetched again when the index changes to the revision it already has", async () => {
  const { requests, scheduler, clock, settle } = setup();
  scheduler.setWanted(open("a"));
  requests[0].resolve(tree(5));
  await settle();
  scheduler.indexChanged(5);
  clock.advance(2000);
  assert.equal(requests.length, 1);
});

test("a tree made while the code moved is stale at once: it is fetched again after the debounce", async () => {
  const { requests, scheduler, clock, settle } = setup();
  scheduler.indexChanged(1);
  scheduler.setWanted(open("a"));
  clock.advance(800); // nothing held yet: nothing to do
  scheduler.indexChanged(2); // while the request is out
  clock.advance(800);
  requests[0].resolve(tree(1)); // it answers for revision 1
  await settle();
  clock.advance(800);
  assert.equal(requests.length, 2, "asked again");
});

test("KEEP counts only the trees nothing wants: a wanted tree is never dropped to make room", async () => {
  const { requests, scheduler, log, settle } = setup({ keep: 2, concurrency: 10 });
  const keys = ["a", "b", "c", "d", "e"];
  scheduler.setWanted(open(...keys));
  requests.forEach((r) => r.resolve(tree(1)));
  await settle();
  assert.deepEqual(Object.keys(scheduler.all()), keys, "five held and all wanted: none dropped although keep is 2");
  scheduler.setWanted(open("e")); // a, b, c, d are unwanted now: two too many
  assert.deepEqual(Object.keys(scheduler.all()), ["c", "d", "e"], "the two oldest unwanted go; the wanted one stays");
  assert.deepEqual(log.filter((l) => l.startsWith("dropped")), ["dropped a", "dropped b"]);
});

test("a core restart in the middle of a fetch: everything is dropped, the wanted keys are fetched again, and the old answer is ignored", async () => {
  const { requests, scheduler, log, settle } = setup();
  scheduler.setWanted(open("a", "b"));
  requests[0].resolve(tree(40));
  await settle(); // a is held (revision 40 of the old core); b is in flight
  scheduler.restarted();
  assert.equal(requests[1].signal.aborted, true, "the request of the old core is cancelled");
  assert.equal(scheduler.tree("a"), undefined, "its tree is gone: revision 40 cannot be compared with the new core's");
  assert.ok(log.includes("reset"));
  assert.deepEqual(requests.slice(2).map((r) => r.key), ["a", "b"], "both asked again");
  requests[1].resolve(tree(41)); // the old b, too late
  await settle();
  assert.equal(scheduler.tree("b"), undefined, "ignored");
  requests[2].resolve(tree(3));
  requests[3].resolve(tree(3));
  await settle();
  assert.deepEqual([scheduler.tree("a")?.revision, scheduler.tree("b")?.revision], [3, 3]);
});

test("a restart cancels the stale-refetch timer: a revision from before it must not trigger anything after it", async () => {
  const { requests, scheduler, clock, settle } = setup();
  scheduler.setWanted(open("a"));
  requests[0].resolve(tree(1));
  await settle();
  scheduler.indexChanged(40); // the old core
  scheduler.restarted();
  assert.equal(clock.pending, 0);
  clock.advance(5000);
  assert.equal(requests.length, 2, "only the fetch the restart itself made");
  requests[1].resolve(tree(2)); // the new core's revision is lower than 40, and that is fine
  await settle();
  clock.advance(5000);
  assert.equal(requests.length, 2, "revision 2 is not stale against nothing");
});
