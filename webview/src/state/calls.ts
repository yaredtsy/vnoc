// The call trees the host holds, by the key of their entry, and what it is doing about each: a copy, changed only as the host says (`applyCallEvent`).
// Never saved (they mean nothing after a restart). When the core restarts the host drops every tree and says so (`callTreesReset`), and when it drops
// one it says that too (`callTreeDropped`): a revision from before a restart cannot be compared with one from after.

import { create } from "zustand";

import { on, request } from "../host.ts";
import { type CallEvent, type CallState, applyCallEvent, noCalls } from "./callEvents.ts";

export const useCalls = create<CallState>(() => noCalls);

const apply = (event: CallEvent) => useCalls.setState((s) => applyCallEvent(s, event), true);

request("callTrees").then(
  (trees) => apply({ type: "snapshot", trees }),
  () => {},
);
on("callTree", ({ key, result }) => apply({ type: "tree", key, result }));
on("callTreeState", ({ key, state, message }) => apply({ type: "state", key, state, message }));
on("callProbe", ({ key, count, revision, epoch }) => apply({ type: "probe", key, count, revision, epoch }));
on("callProbeDropped", ({ key }) => apply({ type: "probeDropped", key }));
on("callTreeDropped", ({ key }) => apply({ type: "dropped", key }));
on("callTreesReset", () => apply({ type: "reset" }));
