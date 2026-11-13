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

interface ProbeRequest {
  key: string;
  signal: AbortSignal;
  resolve(result: { revision: number; count: number }): void;
  reject(e: Error): void;
}

function setup(options: { concurrency?: number; keep?: number } = {}) {
  const clock = new FakeClock();
  const requests: Request[] = [];
  const probeRequests: ProbeRequest[] = [];
  const log: string[] = [];
  const scheduler = new CallScheduler({
    ...options,
    clock,
    fetch: (key, signal) => new Promise((resolve, reject) => requests.push({ key, signal, resolve, reject })),
    probe: (key, signal) => new Promise((resolve, reject) => probeRequests.push({ key, signal, resolve, reject })),
    onProbe: (key, r) => log.push(`probe ${key}=${r.count}@${r.revision}`),
    onProbeDropped: (key) => log.push(`probe dropped ${key}`),
    onTree: (key, r) => log.push(`tree ${key}@${r.revision}`),
    onState: (key, state, message) => log.push(`${state} ${key}${message ? `: ${message}` : ""}`),
    onDropped: (key) => log.push(`dropped ${key}`),
    onReset: () => log.push("reset"),
  });
  const started = () => requests.map((r) => r.key);
  /** lets the promise callbacks run */
  const settle = () => new Promise((resolve) => setImmediate(resolve));
  const probed = () => probeRequests.map((r) => r.key);
  return { clock, requests, probeRequests, log, scheduler, started, probed, settle };
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

test("a wanted key whose fetch failed is fetched again, once, when the code settles (800 ms after the last change)", async () => {
  const { requests, scheduler, clock, log, settle } = setup();
  scheduler.setWanted(open("a"));
  requests[0].reject(new Error("the driver crashed"));
  await settle();
  assert.equal(scheduler.stateOf("a"), "failed");
  scheduler.indexChanged(2);
  clock.advance(799);
  assert.equal(requests.length, 1, "not before the code has settled");
  clock.advance(1);
  assert.equal(requests.length, 2, "one new fetch");
  requests[1].resolve(tree(2));
  await settle();
  assert.deepEqual([scheduler.stateOf("a"), scheduler.tree("a")?.revision], ["idle", 2]);
  assert.deepEqual(log.slice(-3), ["loading a", "tree a@2", "idle a"]);
});

test("a retry that fails again stays failed: no loop until the code settles again", async () => {
  const { requests, scheduler, clock, settle } = setup();
  scheduler.setWanted(open("a"));
  requests[0].reject(new Error("boom"));
  await settle();
  scheduler.indexChanged(2);
  clock.advance(800);
  requests[1].reject(new Error("boom again"));
  await settle();
  assert.equal(scheduler.stateOf("a"), "failed");
  clock.advance(10_000);
  scheduler.setWanted(open("a"));
  assert.equal(requests.length, 2, "nothing more by itself");
  scheduler.indexChanged(3); // the code settles again: one more try
  clock.advance(800);
  assert.equal(requests.length, 3);
});

test("a key that is not wanted is not retried when the code settles", async () => {
  const { requests, scheduler, clock, settle } = setup();
  scheduler.setWanted(open("a", "b"));
  requests[0].reject(new Error("boom"));
  requests[1].reject(new Error("boom"));
  await settle();
  scheduler.setWanted(open("b")); // a is no longer wanted (and forgets its failure)
  scheduler.indexChanged(2);
  clock.advance(800);
  assert.deepEqual(requests.slice(2).map((r) => r.key), ["b"], "only b");
});

// ── probes (05 §3.1): the lowest kind of work ────────────────────────────────────────────────────────────────────────

test("probes run one at a time, in the order given", async () => {
  const { probeRequests, scheduler, probed, settle } = setup();
  scheduler.setWanted([], ["a", "b", "c"]);
  assert.deepEqual(probed(), ["a"], "never two probes at once");
  probeRequests[0].resolve({ revision: 1, count: 2 });
  await settle();
  assert.deepEqual(probed(), ["a", "b"]);
  probeRequests[1].resolve({ revision: 1, count: 0 });
  await settle();
  assert.deepEqual(probed(), ["a", "b", "c"]);
  assert.deepEqual(scheduler.allProbes(), { a: { revision: 1, count: 2 }, b: { revision: 1, count: 0 } });
});

test("a click while three probes wait goes first: no probe starts while a tree is wanted, and they follow it", async () => {
  const { requests, probeRequests, scheduler, started, probed, settle } = setup();
  scheduler.setWanted(open("click"), ["p1", "p2", "p3"]);
  assert.deepEqual(started(), ["click"]);
  assert.deepEqual(probed(), [], "the probes wait");
  requests[0].resolve(tree(1));
  await settle();
  assert.deepEqual(probed(), ["p1"], "then they start, one");
  assert.equal(probeRequests.length, 1);
});

test("no probe starts while a tree is waiting for a place either (the prefetch included)", async () => {
  const { requests, scheduler, probed, settle } = setup({ concurrency: 1 });
  scheduler.setWanted([...open("a"), prefetch("p")], ["x"]);
  assert.deepEqual(probed(), [], "a is in flight and the prefetch waits behind it");
  requests[0].resolve(tree(1));
  await settle();
  assert.deepEqual(probed(), [], "the prefetch is in flight now");
  requests[1].resolve(tree(1));
  await settle();
  assert.deepEqual(probed(), ["x"], "only when nothing else is wanted");
});

test("a click while a probe is in flight cancels the probe so the click does not queue behind it; the probe is asked again after", async () => {
  const { requests, probeRequests, scheduler, started, probed, settle } = setup();
  scheduler.setWanted([], ["p1", "p2"]);
  assert.equal(probeRequests[0].signal.aborted, false);
  scheduler.setWanted(open("click"), ["p1", "p2"]);
  assert.equal(probeRequests[0].signal.aborted, true, "the probe was in the way");
  assert.deepEqual(started(), ["click"], "and the click started at once");
  probeRequests[0].resolve({ revision: 1, count: 9 }); // too late
  await settle();
  assert.deepEqual(scheduler.allProbes(), {}, "its late answer is dropped");
  requests[0].resolve(tree(1));
  await settle();
  assert.deepEqual(probed(), ["p1", "p1"], "p1 is probed again once the click is done");
});

test("a probe for a card no longer drawn is cancelled and its late answer is dropped", async () => {
  const { probeRequests, scheduler, log, probed, settle } = setup();
  scheduler.setWanted([], ["a", "b"]);
  scheduler.setWanted([], ["b"]); // a scrolled out of the drawn set
  assert.equal(probeRequests[0].signal.aborted, true);
  assert.deepEqual(probed(), ["a", "b"], "b is next");
  probeRequests[0].resolve({ revision: 1, count: 5 });
  await settle();
  assert.equal(scheduler.allProbes().a, undefined);
  assert.ok(!log.some((l) => l.startsWith("probe a")), "never reported");
});

test("a function whose tree is held is not probed: the tree says what a probe would", async () => {
  const { requests, scheduler, probed, settle } = setup();
  scheduler.setWanted(open("a"), ["a", "b"]);
  requests[0].resolve(tree(1));
  await settle();
  assert.deepEqual(probed(), ["b"]);
});

test("a probe that fails is not asked again by itself, and is asked once more when the code settles", async () => {
  const { probeRequests, scheduler, clock, probed, settle } = setup();
  scheduler.setWanted([], ["a"]);
  probeRequests[0].reject(new Error("boom"));
  await settle();
  scheduler.setWanted([], ["a"]);
  assert.deepEqual(probed(), ["a"], "no loop");
  scheduler.indexChanged(2);
  clock.advance(800);
  assert.deepEqual(probed(), ["a", "a"], "the code changed: one more try");
});

test("a probe held for an old revision is asked again 800 ms after the last change while it is drawn; the old count stays until then, and an undrawn one is dropped", async () => {
  const { probeRequests, scheduler, clock, log, probed, settle } = setup();
  scheduler.setWanted([], ["a", "b"]);
  probeRequests[0].resolve({ revision: 1, count: 3 });
  await settle();
  probeRequests[1].resolve({ revision: 1, count: 0 });
  await settle();
  scheduler.setWanted([], ["a"]); // b is no longer drawn
  scheduler.indexChanged(2);
  clock.advance(799);
  assert.deepEqual(probed(), ["a", "b"], "not yet");
  clock.advance(1);
  assert.deepEqual(probed(), ["a", "b", "a"], "a is probed again");
  assert.deepEqual(scheduler.allProbes().a, { revision: 1, count: 3 }, "the old count stays in place meanwhile");
  assert.equal(scheduler.allProbes().b, undefined);
  assert.ok(log.includes("probe dropped b"));
  probeRequests[2].resolve({ revision: 2, count: 4 });
  await settle();
  assert.deepEqual(scheduler.allProbes().a, { revision: 2, count: 4 });
});

test("probes start over after a core restart: the old ones are dropped, the one in flight is cancelled, and what is drawn is probed again", async () => {
  const { probeRequests, scheduler, log, probed, settle } = setup();
  scheduler.setWanted([], ["a", "b"]);
  probeRequests[0].resolve({ revision: 40, count: 3 });
  await settle(); // a held (revision 40 of the old core); b in flight
  scheduler.restarted();
  assert.equal(probeRequests[1].signal.aborted, true);
  assert.deepEqual(scheduler.allProbes(), {});
  assert.ok(log.includes("reset"));
  assert.deepEqual(probed(), ["a", "b", "a"], "a is probed again at once");
  probeRequests[1].resolve({ revision: 41, count: 1 }); // the old b, too late
  await settle();
  assert.equal(scheduler.allProbes().b, undefined);
  probeRequests[2].resolve({ revision: 2, count: 7 });
  await settle();
  assert.deepEqual(scheduler.allProbes().a, { revision: 2, count: 7 });
});
