// The tree and its numbers: a copy of what the core knows, replaced as a whole when the core says the index changed. Never saved.

import { create } from "zustand";

import type { StructureResult } from "@vnoc/shared/messages.ts";
import { on, request } from "../host.ts";

interface Data {
  status: "loading" | "ready" | "error";
  /** why it failed, when `status` is "error" */
  message: string;
  structure: StructureResult | null;
  /** counts the loud refreshes (the Refresh button); the canvas draws itself from scratch on each */
  reloads: number;
}

export const useData = create<Data>(() => ({ status: "loading", message: "", structure: null, reloads: 0 }));

/** The newest ask: an answer to an older one must not replace what a newer one brought (or its error). Not the index revision: a core that was
 *  restarted counts revisions from the start again, and its answers would be dropped for ever. */
let latest = 0;

/** `quiet`: the index changed while the user is looking: swap the data without the loading state, and keep the view where it is. */
export function refresh(quiet = false): void {
  const ask = ++latest;
  if (!quiet) useData.setState({ status: "loading" });
  request("structure").then(
    (structure) => {
      if (ask !== latest) return; // two asks crossed: only the newest one speaks
      useData.setState((s) => ({ status: "ready", message: "", structure, reloads: quiet ? s.reloads : s.reloads + 1 }));
    },
    (e: Error) => {
      if (ask === latest) useData.setState({ status: "error", message: e.message });
    },
  );
}

refresh();
on("indexChanged", () => refresh(true));
