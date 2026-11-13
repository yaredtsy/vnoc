// The call trees the host holds, by the key of their entry, and what it is doing about each: a copy, replaced as the host says. Never saved (they mean
// nothing after a restart). When the core restarts the host drops every tree and says so (`callTreesReset`): so does this copy, because a revision from
// before the restart cannot be compared with one from after.

import { create } from "zustand";

import type { CallTreeResult, Events } from "@vnoc/shared/messages.ts";
import { on, request } from "../host.ts";

type State = Events["callTreeState"];

export const useCalls = create<{ trees: Record<string, CallTreeResult>; states: Record<string, { state: State["state"]; message?: string }> }>(() => ({ trees: {}, states: {} }));

// the trees made before this page opened, then every new one; an event that comes first is newer than the list, so it wins
request("callTrees").then(
  (trees) => useCalls.setState((s) => ({ trees: { ...trees, ...s.trees } })),
  () => {},
);
on("callTree", ({ key, result }) => useCalls.setState((s) => ({ trees: { ...s.trees, [key]: result } })));
on("callTreeState", ({ key, state, message }) => useCalls.setState((s) => ({ states: { ...s.states, [key]: { state, message } } })));
on("callTreesReset", () => useCalls.setState({ trees: {}, states: {} }));
