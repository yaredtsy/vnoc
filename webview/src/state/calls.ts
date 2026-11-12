// The call trees the host holds, by the key of their entry: a copy, replaced as a whole when a new one arrives. Never saved (they mean nothing after a restart).

import { create } from "zustand";

import type { CallTreeResult } from "@vnoc/shared/messages.ts";
import { on, request } from "../host.ts";

export const useCalls = create<{ trees: Record<string, CallTreeResult> }>(() => ({ trees: {} }));

// the trees made before this page opened, then every new one; an event that comes first is newer than the list, so it wins
request("callTrees").then(
  (trees) => useCalls.setState((s) => ({ trees: { ...trees, ...s.trees } })),
  () => {},
);
on("callTree", ({ key, result }) => useCalls.setState((s) => ({ trees: { ...s.trees, [key]: result } })));
