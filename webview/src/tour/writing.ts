// What a component of the tour needs to say that the agent is writing: the tour's beat, from the view and the one fact it does not carry. The words are `shared/tourWriting.ts`.

import { type Beat, beatOf, quietOf } from "@vnoc/shared/tourWriting.ts";
import type { TourView } from "@vnoc/shared/tours.ts";
import { useTourList } from "../sidebar/tours/store.ts";

/** Whether the open tour is quiet (no news): the core's word for it, from the list the host sends again when a tour changes and every 30 s while one is being written. */
export function useQuiet(view: TourView | null): boolean {
  const rows = useTourList((s) => s.rows);
  return quietOf(rows, view);
}

/** The tour's beat: finished, writing, stopping, stopped, or quiet. `finished` for no tour. */
export function useBeat(view: TourView | null): Beat {
  const quiet = useQuiet(view);
  return view ? beatOf(view, quiet) : "finished";
}
