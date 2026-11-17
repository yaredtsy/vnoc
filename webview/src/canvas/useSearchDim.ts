import { useEffect, useMemo, useRef, useState } from "react";

import { newest } from "@vnoc/shared/search.ts";
import { on, post, request } from "../host.ts";
import { useSession } from "../state/session.ts";
import { useView } from "../state/view.ts";
import type { Placed } from "./layout.ts";
import { type Dim, dimmedCards, keysOf } from "./searchDim.ts";

/** How long the cards must stay as they are before the canvas asks again (panning and opening move them many times a second). A new query asks at once. */
export const CARDS_SETTLE_MS = 50;

/**
 * While the sidebar's box has text (`session.search`), asks the core which of this canvas's cards the query finds (`searchCards`: every laid-out card, on screen or
 * not), and says which to dim in the view store, where each card reads it. Only the answer to the newest request is applied (`newest`); until one arrives nothing is
 * dimmed, and when the search ends everything is bright at once. Dims nothing itself: what matches is the core's. Posts `searchDim` for the test suite.
 */
export function useSearchDim(cards: readonly Placed[]): void {
  const search = useSession((s) => s.search);
  const selected = useSession((s) => s.contexts[s.active].selected);
  const [dim, setDim] = useState<{ query: string; dim: Dim } | null>(null);
  const [numbering] = useState(newest);
  // the index changed: the answer may be different now, asked again like a change of the cards
  const [revision, setRevision] = useState(0);
  useEffect(() => on("indexChanged", () => setRevision((n) => n + 1)), []);

  const keys = useMemo(() => keysOf(cards), [cards]);
  const lastQuery = useRef<string | null>(null);
  useEffect(() => {
    if (search === null) {
      lastQuery.current = null;
      numbering.drop();
      setDim(null);
      return;
    }
    const ask = () => {
      lastQuery.current = search;
      const number = numbering.next();
      request("searchCards", { query: search, ids: keys }).then(
        (answer) => numbering.isNewest(number) && setDim({ query: search, dim: { asked: new Set(keys), hits: new Set(answer.hits) } }),
        () => {}, // the core said no (it is not running): the dim stays as it was, and the next change asks again
      );
    };
    if (lastQuery.current !== search) return ask();
    const timer = setTimeout(ask, CARDS_SETTLE_MS);
    return () => clearTimeout(timer);
  }, [search, keys, revision]);

  const dimmed = useMemo(() => dimmedCards(cards, dim?.dim ?? null, selected), [cards, dim, selected]);
  // (a write to the view store saves the view: nothing is written when nothing changes)
  useEffect(() => {
    if (dimmed.length === 0 && useView.getState().dimmed === null) return;
    useView.setState({ dimmed: dimmed.length === 0 ? null : new Set(dimmed) });
  }, [dimmed]);
  // the canvas going away leaves nothing dim for the next one
  useEffect(
    () => () => {
      if (useView.getState().dimmed !== null) useView.setState({ dimmed: null });
    },
    [],
  );

  // an answer was applied (or the search ended): the test suite's note
  const posted = useRef<string>(JSON.stringify([null, []])); // a canvas with no search says nothing when it starts
  useEffect(() => {
    const note = JSON.stringify([dim?.query ?? null, dimmed]);
    if (note === posted.current) return;
    posted.current = note;
    if (dim === null && search !== null) return; // asked, not answered: nothing to say yet
    post({ type: "searchDim", query: dim?.query ?? "", dimmed });
  }, [dim, dimmed, search]);
}
