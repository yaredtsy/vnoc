// Which call trees the host fetches, and when (docs/16-ui-redesign/05-calls-as-nodes.md §2 and §5). A plain class: the request and the clock are
// given to it, so it is tested with a fake of each and no VS Code. `extension/src/callTree.ts` wires it to the core.

import type { Wanted } from "./calls.ts";
import type { CallTreeResult } from "./messages";

export interface Clock {
  setTimeout(fn: () => void, ms: number): unknown;
  clearTimeout(handle: unknown): void;
}

export interface SchedulerOptions {
  /** the request for one entry's tree; `signal` fires when nothing wants it any more (or the core restarted) */
  fetch(key: string, signal: AbortSignal): Promise<CallTreeResult>;
  clock: Clock;
  /** a tree arrived (a new one replaces the old one) */
  onTree(key: string, result: CallTreeResult): void;
  /** what is happening for this entry: `loading` (a request is out, an older tree may still be held), `failed`, or `idle` */
  onState(key: string, state: "loading" | "failed" | "idle", message?: string): void;
  /**
   * The probe of one function: how many call cards opening it would show (`vnoc/hasCalls`). The lowest kind of work: one at a time, and only while
   * no tree is being fetched or waits for a place, because the driver solves on one worker and a probe in front of a click delays the click.
   */
  probe(key: string, signal: AbortSignal): Promise<{ revision: number; count: number }>;
  /** a probe answered */
  onProbe(key: string, result: { revision: number; count: number }): void;
  /** a held probe was dropped: its function is not drawn and the code moved on */
  onProbeDropped(key: string): void;
  /** a held tree was dropped: it was stale and nothing wanted it, or it was the oldest unwanted one */
  onDropped(key: string): void;
  /** the core was restarted: every tree is gone */
  onReset(): void;
  /** requests in flight at once; the prefetch waits for a free place */
  concurrency?: number;
  /** the most trees nothing wants that are kept: a wanted tree is never dropped to make room */
  keep?: number;
  /** a stale tree that is wanted is fetched again this long after the LAST index change */
  debounceMs?: number;
}

interface Running {
  id: number;
  controller: AbortController;
  /** the index revision when it started: a newer one by the time it answers makes its tree stale at once */
  revisionAtStart: number | null;
}

export class CallScheduler {
  private readonly held = new Map<string, CallTreeResult>();
  private wanted: Wanted[] = [];
  private readonly running = new Map<string, Running>();
  private readonly failed = new Map<string, string>();
  /** held trees that are stale and wanted: fetched again, the old one stays in place until the new one is here */
  private readonly refetch = new Set<string>();
  private revision: number | null = null;
  private timer: unknown;
  private nextId = 0;
  // probes (see `SchedulerOptions.probe`)
  private probeWanted: string[] = [];
  private readonly probes = new Map<string, { revision: number; count: number }>();
  private probing: { key: string; id: number; controller: AbortController; revisionAtStart: number | null } | null = null;
  /** held probes that are stale and still drawn: asked again, the old count stays in place until the new one is here */
  private readonly probeRefetch = new Set<string>();
  /** probes that failed since the code last settled: not asked again until it does */
  private readonly probeFailed = new Set<string>();

  private readonly o: SchedulerOptions;

  constructor(options: SchedulerOptions) {
    this.o = options;
  }

  private get concurrency() {
    return this.o.concurrency ?? 2;
  }
  private get keep() {
    return this.o.keep ?? 50;
  }
  private get debounceMs() {
    return this.o.debounceMs ?? 800;
  }

  /** The trees held, newest last. */
  all(): Record<string, CallTreeResult> {
    return Object.fromEntries(this.held);
  }

  tree(key: string): CallTreeResult | undefined {
    return this.held.get(key);
  }

  stateOf(key: string): "loading" | "failed" | "idle" {
    return this.running.has(key) ? "loading" : this.failed.has(key) ? "failed" : "idle";
  }

  /** The probes held, by key. */
  allProbes(): Record<string, { revision: number; count: number }> {
    return Object.fromEntries(this.probes);
  }

  /**
   * What is wanted now (after every session or structure change). Cancels what is in flight for a key no longer wanted; fetches what is missing.
   * `probeKeys`: the function and class cards drawn, the one to look at first first; each is probed unless its tree is held.
   */
  setWanted(list: readonly Wanted[], probeKeys: readonly string[] = []): void {
    this.wanted = [...list];
    this.probeWanted = [...new Set(probeKeys)];
    if (this.probing && !this.probeWanted.includes(this.probing.key)) this.cancelProbe(); // no longer drawn: its late answer is dropped
    for (const key of [...this.probeFailed]) if (!this.probeWanted.includes(key)) this.probeFailed.delete(key);
    for (const key of [...this.probeRefetch]) if (!this.probeWanted.includes(key)) this.probeRefetch.delete(key);
    const keys = new Set(list.map((w) => w.key));
    for (const key of [...this.running.keys()]) if (!keys.has(key)) this.cancel(key);
    for (const key of [...this.failed.keys()]) {
      if (keys.has(key)) continue;
      this.failed.delete(key); // no longer wanted: wanting it again asks again
      this.o.onState(key, "idle");
    }
    for (const key of [...this.refetch]) if (!keys.has(key)) this.refetch.delete(key);
    this.evict();
    this.pump();
  }

  /** The index changed to this revision. Stale trees are dealt with 800 ms after the last change (typing makes many). */
  indexChanged(revision: number): void {
    this.revision = revision;
    this.arm();
  }

  /** Fetches again a key whose last request failed. */
  retry(key: string): void {
    if (!this.failed.delete(key)) return;
    this.pump();
  }

  /** The core was restarted: its revisions start over, so no tree and no request from before may be compared with what comes after. Everything is dropped; the wanted keys are fetched again. */
  restarted(): void {
    for (const r of this.running.values()) r.controller.abort();
    this.running.clear();
    this.held.clear();
    this.failed.clear();
    this.refetch.clear();
    this.probing?.controller.abort();
    this.probing = null;
    this.probes.clear();
    this.probeRefetch.clear();
    this.probeFailed.clear();
    this.revision = null;
    if (this.timer !== undefined) this.o.clock.clearTimeout(this.timer);
    this.timer = undefined;
    this.o.onReset();
    this.pump();
  }

  // ── inside ──────────────────────────────────────────────────────────────────────────────────────────────────────

  /**
   * Starts what is missing, up to `concurrency`: first what is open, the prefetch last. A probe is started only when nothing else is in flight or
   * waits for a place; a probe that is running when a tree is wanted is cancelled, so the tree never queues behind it (it is asked again after).
   */
  private pump(): void {
    const order = [...this.wanted.filter((w) => w.reason !== "prefetch"), ...this.wanted.filter((w) => w.reason === "prefetch")];
    let waiting = false;
    for (const { key } of order) {
      if (this.running.has(key) || this.failed.has(key)) continue;
      if (this.held.has(key) && !this.refetch.has(key)) continue;
      if (this.running.size >= this.concurrency) {
        waiting = true;
        break;
      }
      this.cancelProbe();
      this.start(key);
    }
    if (waiting || this.running.size > 0 || this.probing) return;
    for (const key of this.probeWanted) {
      if (this.probeFailed.has(key)) continue;
      if (this.held.has(key) && !this.refetch.has(key)) continue; // its tree is here: it says what a probe would
      if (this.probes.has(key) && !this.probeRefetch.has(key)) continue;
      this.startProbe(key);
      return;
    }
  }

  private startProbe(key: string): void {
    const id = ++this.nextId;
    const controller = new AbortController();
    this.probing = { key, id, controller, revisionAtStart: this.revision };
    const still = () => this.probing?.id === id;
    this.o.probe(key, controller.signal).then(
      (result) => {
        if (!still()) return;
        const { revisionAtStart } = this.probing!;
        this.probing = null;
        this.probeRefetch.delete(key);
        this.probes.set(key, result);
        this.o.onProbe(key, result);
        if (this.revision !== null && revisionAtStart !== this.revision) this.arm();
        this.pump();
      },
      () => {
        if (!still()) return;
        this.probing = null;
        this.probeFailed.add(key); // not asked again until the code settles: no loop of failing probes
        this.pump();
      },
    );
  }

  /** Stops the probe in flight, if any. Its key stays wanted and is asked again when nothing else is. */
  private cancelProbe(): void {
    if (!this.probing) return;
    this.probing.controller.abort();
    this.probing = null;
  }

  private start(key: string): void {
    const id = ++this.nextId;
    const controller = new AbortController();
    this.running.set(key, { id, controller, revisionAtStart: this.revision });
    this.o.onState(key, "loading");
    // `still`: only the newest request for a key may speak; one that was cancelled (or a restart's victim) is no longer `running`
    const still = () => this.running.get(key)?.id === id;
    this.o.fetch(key, controller.signal).then(
      (result) => {
        if (!still()) return;
        const { revisionAtStart } = this.running.get(key)!;
        this.running.delete(key);
        this.refetch.delete(key);
        this.held.delete(key); // a new one is the newest again
        this.held.set(key, result);
        this.evict();
        this.o.onTree(key, result);
        this.o.onState(key, "idle");
        if (this.revision !== null && revisionAtStart !== this.revision) this.arm(); // the code moved while it was being made
        this.pump();
      },
      (error: unknown) => {
        if (!still()) return;
        this.running.delete(key);
        this.refetch.delete(key); // a failed refetch keeps the old tree; asking again is `retry`
        const message = String((error as Error)?.message ?? error);
        this.failed.set(key, message);
        this.o.onState(key, "failed", message);
        this.pump();
      },
    );
  }

  private cancel(key: string): void {
    const r = this.running.get(key);
    if (!r) return;
    this.running.delete(key);
    r.controller.abort();
    this.o.onState(key, "idle");
  }

  private arm(): void {
    if (this.timer !== undefined) this.o.clock.clearTimeout(this.timer);
    this.timer = this.o.clock.setTimeout(() => {
      this.timer = undefined;
      this.settleStale();
    }, this.debounceMs);
  }

  /**
   * The code settled. A stale tree that is wanted is fetched again (the old one stays on screen); one nothing wants is dropped. A wanted key whose
   * last fetch failed is asked again, once: the code changed, so the failure may not repeat. If it fails again it stays failed until the code settles
   * again (or `retry`); a key nothing wants is not retried.
   */
  private settleStale(): void {
    if (this.revision === null) return;
    const wanted = new Set(this.wanted.map((w) => w.key));
    for (const key of [...this.failed.keys()]) {
      if (!wanted.has(key)) continue;
      this.failed.delete(key); // `pump` asks again; a second failure puts it back
    }
    this.probeFailed.clear();
    for (const [key, probe] of [...this.probes]) {
      if (probe.revision >= this.revision) continue;
      if (this.probeWanted.includes(key)) this.probeRefetch.add(key);
      else {
        this.probes.delete(key);
        this.o.onProbeDropped(key);
      }
    }
    for (const [key, tree] of [...this.held]) {
      if (tree.revision >= this.revision) continue;
      if (wanted.has(key)) this.refetch.add(key);
      else this.drop(key);
    }
    this.pump();
  }

  /** Over `keep` trees that nothing wants: the oldest go first. A wanted tree is never dropped here. */
  private evict(): void {
    const wanted = new Set(this.wanted.map((w) => w.key));
    const unwanted = [...this.held.keys()].filter((key) => !wanted.has(key));
    for (const key of unwanted.slice(0, Math.max(0, unwanted.length - this.keep))) this.drop(key);
  }

  private drop(key: string): void {
    if (this.held.delete(key)) this.o.onDropped(key);
  }
}
