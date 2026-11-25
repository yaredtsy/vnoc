import { useEffect, useState } from "react";

import type { TourPinAnswer, TourView } from "@vnoc/shared/tours.ts";
import { request } from "../../host.ts";
import { dryKey, wantsDryPin } from "../pin.ts";

/**
 * Why a tour is not pinned, from the core (`tourPin` with `dry`: nothing is written), for the cover. Asked when the cover is shown for a finished tour that is not pinned, and again only
 * when what the answer reads changed (`dryKey`: not on every `tour` event, which a move of the reader also is): the core answers one request per CPU at once, and this one checks the
 * tour against the code. The answer held is shown while the next is awaited, so the line does not flash. Undefined until the first answer; null when the core could not say.
 */
const held = new Map<string, TourPinAnswer | null>();

export function useDryPin(view: TourView | null, shown: boolean): TourPinAnswer | null | undefined {
  const [, again] = useState(0);
  const wanted = shown && view !== null && wantsDryPin(view);
  const key = view !== null ? dryKey(view) : "";
  const id = view?.id ?? "";
  useEffect(() => {
    if (!wanted) return;
    let live = true;
    request("tourPin", { id, dry: true }).then(
      (answer) => live && (held.set(id, answer), again((n) => n + 1)),
      () => live && (held.set(id, null), again((n) => n + 1)),
    );
    return () => {
      live = false;
    };
  }, [wanted, key]); // eslint-disable-line react-hooks/exhaustive-deps
  return wanted ? held.get(id) : undefined;
}
