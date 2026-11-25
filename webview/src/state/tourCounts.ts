// How many tours pass through each node, by the structure's key (`vnoc/tourCounts`): the `▶ N` of a card and of a row. The host holds the answer and sends it again when a tour
// or the index changes. Nothing is counted here.

import { create } from "zustand";

import { on, request } from "../host.ts";

export const useTourCounts = create<{ counts: Readonly<Record<string, number>> }>(() => ({ counts: {} }));

request("tourCounts").then(
  ({ counts }) => useTourCounts.setState({ counts }),
  () => {},
);
on("tourCounts", ({ counts }) => useTourCounts.setState({ counts }));

/** The tours through a node: 0 for none, and for a node with no key (a call to a function the project does not have). */
export const toursOf = (counts: Readonly<Record<string, number>>, key: string | null): number => (key === null ? 0 : (counts[key] ?? 0));
