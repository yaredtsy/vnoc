import * as vscode from "vscode";

import { wantedCalls } from "@vnoc/shared/calls.ts";
import { CallScheduler } from "@vnoc/shared/callScheduler.ts";
import type { CallTreeResult, TreeNode } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import type { Core } from "./core";

/** What happened to the call trees, for the window suite (`callTreeLog`): a state change, a tree, a reset. */
export type CallLogEntry = { kind: "state"; key: string; state: "loading" | "failed" | "idle"; message?: string } | { kind: "tree"; key: string; revision: number; epoch?: number } | { kind: "dropped"; key: string } | { kind: "reset" };

export interface CallTreeEvents {
  /** a tree arrived: tell the open canvases (and the session, which drops what no longer resolves) */
  tree(key: string, result: CallTreeResult): void;
  /** the host's work on this entry changed: the canvases draw spinners and failures from it */
  state(key: string, state: "loading" | "failed" | "idle", message?: string): void;
  /** every tree is gone (the core restarted): the canvases drop theirs */
  reset(): void;
}

/**
 * The call trees the host holds and the fetching of them (docs/16-ui-redesign/05-calls-as-nodes.md §2): the host asks for what the session has open
 * (`wantedCalls`), two at a time, cancels what stops being wanted and fetches a stale tree again when the code settles. The decisions are
 * `shared/callScheduler.ts`'s; this class gives it the core as its request and the timers as its clock, and tells the canvases what happened.
 * Trees are not saved: they are large and mean nothing after a restart.
 */
export class CallTrees {
  private readonly scheduler: CallScheduler;
  private readonly loading = new Set<string>();
  private endProgress: (() => void) | undefined;
  private holdMs = 0;
  private readonly waiting = new Map<string, ((r: Settled) => void)[]>();
  /** what happened, newest last (the window suite reads it) */
  readonly log: CallLogEntry[] = [];

  constructor(core: Core, private readonly events: CallTreeEvents) {
    this.scheduler = new CallScheduler({
      clock: { setTimeout: (fn, ms) => setTimeout(fn, ms), clearTimeout: (handle) => clearTimeout(handle as NodeJS.Timeout) },
      fetch: async (key, signal) => {
        if (this.holdMs > 0) await sleep(this.holdMs, signal); // the window suite makes the answer slow enough to cancel
        const source = new vscode.CancellationTokenSource();
        const cancel = () => source.cancel(); // `$/cancelRequest`: the core stops the work under it
        signal.addEventListener("abort", cancel);
        try {
          const result = await core.request<CallTreeResult>("vnoc/callTree", { key }, source.token);
          return { ...result, epoch: core.epoch };
        } finally {
          signal.removeEventListener("abort", cancel);
          source.dispose();
        }
      },
      onTree: (key, result) => {
        this.note({ kind: "tree", key, revision: result.revision, epoch: result.epoch });
        this.events.tree(key, result);
        this.settle(key, { state: "idle", result });
      },
      onState: (key, state, message) => {
        this.note({ kind: "state", key, state, message });
        this.events.state(key, state, message);
        this.progress(key, state === "loading");
        if (state === "failed") this.settle(key, { state: "failed", message: message ?? "" });
      },
      onDropped: (key) => this.note({ kind: "dropped", key }),
      onReset: () => {
        this.note({ kind: "reset" });
        this.loading.clear();
        this.endProgress?.();
        this.events.reset();
      },
    });
  }

  all(): Record<string, CallTreeResult> {
    return this.scheduler.all();
  }

  /** What the session has open or selected changed, or a tree arrived: ask for what is wanted now. */
  update(session: Session, tree: TreeNode): void {
    this.scheduler.setWanted(wantedCalls(session, tree, this.scheduler.all()));
  }

  /** The core's index changed: stale trees are dealt with 800 ms after the last change. */
  indexChanged(revision: number): void {
    this.scheduler.indexChanged(revision);
  }

  /** The core was restarted: drop every tree, fetch the wanted ones again. */
  restarted(): void {
    this.scheduler.restarted();
  }

  retry(key: string): void {
    this.scheduler.retry(key);
  }

  /** Resolves when the entry's tree has arrived, or its request failed. It does not resolve when the request is cancelled. */
  settled(key: string): Promise<Settled> {
    const held = this.scheduler.tree(key);
    if (held && this.scheduler.stateOf(key) === "idle") return Promise.resolve({ state: "idle", result: held });
    return new Promise((resolve) => this.waiting.set(key, [...(this.waiting.get(key) ?? []), resolve]));
  }

  /** For the window suite: every answer takes at least this long, so a test can cancel a request before it is made. */
  hold(ms: number): void {
    this.holdMs = ms;
  }

  private settle(key: string, outcome: Settled): void {
    for (const resolve of this.waiting.get(key) ?? []) resolve(outcome);
    this.waiting.delete(key);
  }

  private note(entry: CallLogEntry): void {
    this.log.push(entry);
    if (this.log.length > 500) this.log.splice(0, this.log.length - 500);
  }

  /** The view's own thin progress line while anything is being fetched: no notification, a click on a chevron is not worth a toast. */
  private progress(key: string, on: boolean): void {
    const before = this.loading.size;
    if (on) this.loading.add(key);
    else this.loading.delete(key);
    if (before === 0 && this.loading.size > 0) {
      void vscode.window.withProgress({ location: { viewId: "vnoc.structure" } }, () => new Promise<void>((resolve) => (this.endProgress = resolve)));
    } else if (before > 0 && this.loading.size === 0) {
      this.endProgress?.();
    }
  }
}

export type Settled = { state: "idle"; result: CallTreeResult } | { state: "failed"; message: string };

/** A sleep that ends early when the signal fires (the request was cancelled before it was made). */
const sleep = (ms: number, signal: AbortSignal) =>
  new Promise<void>((resolve, reject) => {
    const timer = setTimeout(resolve, ms);
    signal.addEventListener("abort", () => (clearTimeout(timer), reject(new Error("cancelled"))), { once: true });
  });
