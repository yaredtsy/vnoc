// What the page knows about call trees, and how each message from the host changes it. Pure, so the table of messages is tested without a browser;
// `calls.ts` (the store) only calls `applyCallEvent`.

import type { CallTreeResult } from "@vnoc/shared/messages.ts";

export interface CallState {
  trees: Record<string, CallTreeResult>;
  /** how many call cards opening each function would show, from the host's probes */
  probes: Record<string, { count: number; revision: number; epoch?: number }>;
  /** what the host is doing about each entry */
  states: Record<string, { state: "loading" | "failed" | "idle"; message?: string }>;
}

export const noCalls: CallState = { trees: {}, probes: {}, states: {} };

export type CallEvent =
  | { type: "tree"; key: string; result: CallTreeResult }
  | { type: "state"; key: string; state: "loading" | "failed" | "idle"; message?: string }
  /** a probe answered */
  | { type: "probe"; key: string; count: number; revision: number; epoch?: number }
  /** the host dropped its probe of this function */
  | { type: "probeDropped"; key: string }
  /** the host dropped the tree: stale and nothing wants it, or the oldest unwanted one */
  | { type: "dropped"; key: string }
  /** the core restarted: every tree from before is meaningless */
  | { type: "reset" }
  /** the trees the host held when this page asked; an event that came first is newer than the list, so it wins */
  | { type: "snapshot"; trees: Record<string, CallTreeResult> };

/** The same object when nothing changes, so a store that compares by identity does not draw again. */
export function applyCallEvent(s: CallState, e: CallEvent): CallState {
  switch (e.type) {
    case "tree":
      return { ...s, trees: { ...s.trees, [e.key]: e.result } };
    case "state": {
      const old = s.states[e.key];
      return old?.state === e.state && old.message === e.message ? s : { ...s, states: { ...s.states, [e.key]: { state: e.state, message: e.message } } };
    }
    case "probe":
      return { ...s, probes: { ...s.probes, [e.key]: { count: e.count, revision: e.revision, epoch: e.epoch } } };
    case "probeDropped": {
      if (!(e.key in s.probes)) return s;
      const { [e.key]: _probe, ...probes } = s.probes;
      return { ...s, probes };
    }
    case "dropped": {
      if (!(e.key in s.trees) && !(e.key in s.states)) return s;
      const { [e.key]: _tree, ...trees } = s.trees;
      const { [e.key]: _state, ...states } = s.states;
      return { ...s, trees, states };
    }
    case "reset":
      return s === noCalls || (Object.keys(s.trees).length + Object.keys(s.states).length + Object.keys(s.probes).length === 0) ? s : { trees: {}, probes: {}, states: {} };
    case "snapshot":
      return { ...s, trees: { ...e.trees, ...s.trees } };
  }
}
