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

/** `quiet`: the index changed while the user is looking: swap the data without the loading state, and keep the view where it is. */
export function refresh(quiet = false): void {
  if (!quiet) useData.setState({ status: "loading" });
  request("structure").then(
    (structure) => {
      const current = useData.getState().structure;
      if (current && structure.revision < current.revision) return; // two asks crossed: the older answer must not replace the newer one
      useData.setState((s) => ({ status: "ready", structure, reloads: quiet ? s.reloads : s.reloads + 1 }));
    },
    (e: Error) => useData.setState({ status: "error", message: e.message }),
  );
}

refresh();
on("indexChanged", () => refresh(true));
