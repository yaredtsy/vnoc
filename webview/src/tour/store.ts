// The webview's copy of the open tour (docs/26-tours/05-on-screen.md §5): what the host holds, as it sent it. Read-only here: the place moves by `tourGo` through the
// host, and the host sends the view back. Nothing is kept that the host does not have.

import { create } from "zustand";

import type { TourId, TourUnreadable, TourView } from "@vnoc/shared/tours.ts";
import { on, request } from "../host.ts";

interface Held {
  id: TourId | null;
  view: TourView | TourUnreadable | null;
}

export const useTour = create<Held>(() => ({ id: null, view: null }));

on("tour", ({ id, view }) => useTour.setState({ id, view }));

/** Asks the host for a tour: it answers from what it holds when that is the open tour. An answer for a tour that is not the one wanted any more is dropped. */
export function loadTour(id: TourId, still: () => boolean): void {
  request("tour", { id }).then(
    (view) => still() && useTour.setState({ id, view }),
    (e: Error) => still() && useTour.setState({ id, view: { unreadable: e.message } }),
  );
}

/** The moves are sent one after the other: each is made against the place the one before left (`next`, `next`), and two sent together are two requests that can overtake each other. */
let moves: Promise<unknown> = Promise.resolve();

/** A move for the core, by the host: `next`, `prev`, `cover`, `end`, or a jump to a stop (`s4`). The view comes back as the host's `tour` event. */
export function go(id: TourId, move: string): void {
  moves = moves.then(() => request("tourGo", { id, move }).catch(() => {}));
}

/** ■ Stop: the core writes the halt (the agent answers it at its next command); the host asks for the tour again, and the view's state says `stopping`. */
export function halt(id: TourId): void {
  void request("tourHalt", { id }).catch(() => {});
}
