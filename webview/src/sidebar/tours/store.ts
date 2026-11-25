// The list of tours, as the host sent it (the core's order, newest first): asked once when the page starts, then the host sends the whole list whenever the core says
// a tour changed. Nothing is sorted or counted here.

import { create } from "zustand";

import type { TourRow } from "@vnoc/shared/tours.ts";
import { on, request } from "../../host.ts";

export const useTourList = create<{ rows: TourRow[] | null }>(() => ({ rows: null }));

request("tours", undefined).then(
  (rows) => useTourList.setState({ rows }),
  () => useTourList.setState({ rows: [] }),
);
on("tours", ({ tours }) => useTourList.setState({ rows: tours }));
