import { useEffect, useState } from "react";

import type { AsWritten } from "@vnoc/shared/tours.ts";
import { on, request } from "../../host.ts";

/**
 * A stop as it was written (`tourCode`), kept in the page per (tour, stop). The answer is kept until the next `tour` event (the view changed, and with it perhaps the pin): then it is
 * asked for again, and the one held is shown meanwhile, so a step does not flash empty. Undefined while the first answer is awaited; null when the core has none.
 */
const held = new Map<string, AsWritten | null>();
let generation = 0;
const listeners = new Set<() => void>();
const key = (tour: string, stop: string) => `${tour}|${stop}`;

on("tour", () => {
  generation++;
  for (const l of listeners) l();
});

export function useAsWritten(tour: string, stop: string | null, wanted: boolean): AsWritten | null | undefined {
  const [, again] = useState(0);
  const [seen, setSeen] = useState(generation);
  useEffect(() => {
    const l = () => setSeen(generation);
    listeners.add(l);
    return () => void listeners.delete(l);
  }, []);
  useEffect(() => {
    if (!wanted || stop === null) return;
    let live = true;
    request("tourCode", { id: tour, stop }).then(
      (answer) => live && (held.set(key(tour, stop), answer), again((n) => n + 1)),
      () => live && (held.set(key(tour, stop), null), again((n) => n + 1)),
    );
    return () => {
      live = false;
    };
  }, [tour, stop, wanted, seen]);
  return wanted && stop !== null ? held.get(key(tour, stop)) : undefined;
}
