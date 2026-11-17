import { useEffect, useRef, useState } from "react";

import type { SearchNode, SearchResult } from "@vnoc/shared/messages.ts";
import { on, post, request } from "../host.ts";
import { dispatch, useSession } from "../state/session.ts";
import { pickIntent, searchKey } from "./search.ts";

/** How long the box must be still before the page asks the core (05 §1). */
const DEBOUNCE_MS = 50;

/** An answer that was drawn: the text it answers, and when the box changed and the request was sent (for the test suite's note). */
interface Answer {
  query: string;
  result: SearchResult;
  changedAt: number;
  sentAt: number;
}

/**
 * The search box's state and what it does (06 §5, docs/20-search): the query, the core's rows, the active row. While there is a query the panels give way to
 * the rows; the active row is pointed at on the canvas (`highlight`: nothing is selected); `Enter` or a click picks it; `Esc` clears. `focusList` gives the
 * keyboard to the active panel's list when the search ends. Keeps no state a gesture could need elsewhere: the session has the rest.
 *
 * The core finds and ranks. The page numbers its requests and draws only the answer to the newest (Q8); it does not compare the answer's `revision` (Q5: a
 * restarted core counts from the start again). While an answer is on its way the last rows stay; an empty box asks nothing and clears them at once.
 */
export function useSearch(focusList: () => void) {
  const [query, setQueryState] = useState("");
  const [active, setActive] = useState(0);
  const [answer, setAnswer] = useState<Answer | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const toTree = useRef(false);
  const searching = query.trim() !== "";
  const rows: SearchNode[] = searching ? (answer?.result.nodes ?? []) : [];

  /** The number of the newest request: an answer to an older one is dropped. Moves on when the box empties, so an answer on its way then is dropped too. */
  const newest = useRef(0);
  /** When the box last changed. */
  const changedAt = useRef(0);
  const latestQuery = useRef(query);
  latestQuery.current = query;

  const ask = (text: string) => {
    const number = ++newest.current;
    const sentAt = performance.now();
    const started = changedAt.current;
    request("search", { query: text }).then(
      (result) => {
        if (number !== newest.current) return;
        setAnswer({ query: text, result, changedAt: started, sentAt });
        setActive((a) => Math.min(a, Math.max(result.nodes.length - 1, 0)));
      },
      () => {}, // the core said no (it is not running): the last rows stay, and the next change asks again
    );
  };
  const askLatest = useRef(ask);
  askLatest.current = ask;

  const setQuery = (value: string) => {
    changedAt.current = performance.now();
    setQueryState(value);
    setActive(0);
  };

  // each change of the box asks the core after a pause; an empty box asks nothing and drops what was on its way
  useEffect(() => {
    if (!searching) {
      newest.current++;
      setAnswer(null);
      return;
    }
    const timer = setTimeout(() => askLatest.current(query), DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query, searching]);

  // the tree changed while the box has text: the rows may be different now
  useEffect(
    () =>
      on("indexChanged", () => {
        if (latestQuery.current.trim() === "") return;
        changedAt.current = performance.now();
        askLatest.current(latestQuery.current);
      }),
    [],
  );

  // the test suite cannot type in a webview: `setSearch` is the text a person's typing would leave in the box
  useEffect(() => on("viewCommand", ({ name, text }) => name === "setSearch" && setQuery(text ?? "")), []);

  // an answer was drawn (this runs after the commit that drew it): the test suite's note
  useEffect(() => {
    if (!answer) return;
    const now = performance.now();
    post({ type: "searchDrawn", query: answer.query, ids: answer.result.nodes.map((n) => n.id), total: answer.result.nodes_total, ms: now - answer.changedAt, trip: now - answer.sentAt });
  }, [answer]);

  // the active row's card is pointed at on the canvas; no query, nothing pointed at
  const told = useRef<string | null>(null);
  useEffect(() => {
    const id = searching ? (rows[active]?.id ?? null) : null;
    if (id === told.current) return;
    told.current = id;
    dispatch({ type: "highlight", id });
  }, [searching, rows, active]);
  useEffect(
    () => () => {
      if (told.current !== null) dispatch({ type: "highlight", id: null });
    },
    [],
  );

  // when the search ends by a gesture in the box, the keyboard goes to the list that comes back
  const focusLatest = useRef(focusList);
  focusLatest.current = focusList;
  useEffect(() => {
    if (searching || !toTree.current) return;
    toTree.current = false;
    focusLatest.current();
  }, [searching]);

  const clear = () => {
    toTree.current = true;
    setQuery("");
  };
  /** Reveal in the active context (the focus shortens if it hides the node, 03 §3), then the click rule of 07 §4. The box clears; the list comes back with the row in view. */
  const pick = (row: SearchNode) => {
    const context = useSession.getState().active;
    dispatch(pickIntent(row.id));
    void request("rowClick", { context, id: row.id }).catch(() => {});
    clear();
  };
  const onKeyDown = (e: React.KeyboardEvent) => {
    const key = searchKey(e.key, rows.length, active);
    if (!key || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    e.preventDefault();
    if (key.type === "move") setActive(key.to);
    else if (key.type === "pick") pick(rows[active]);
    else clear();
  };
  /** `/` and `Ctrl/Cmd+F` from anywhere in the view: the cursor goes into the box. */
  const focusBox = () => {
    input.current?.focus();
    input.current?.select();
  };
  /** The answer for the box's text came and found nothing (not: no answer yet). */
  const noMatches = searching && answer?.query === query && rows.length === 0;
  return { query, setQuery, rows, total: answer?.result.nodes_total ?? 0, noMatches, active, setActive, searching, input, onKeyDown, pick, clear, focusBox };
}
