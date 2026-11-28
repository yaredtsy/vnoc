import { useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";

import type { SearchDoc, SearchNode, SearchResult } from "@vnoc/shared/messages.ts";
import { newest, outcomeOf } from "@vnoc/shared/search.ts";
import { on, post, request } from "../host.ts";
import { dispatch, useSession } from "../state/session.ts";
import { pointedAt, rowAt, searchKey } from "./search.ts";

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
 * The core finds and ranks, and says in `notes` what was wrong with a filter of the query (drawn under the box, with the rows or without). The page asks on each change of the box (no debounce: a round trip is about 5 ms) and draws only the answer to the newest request (Q8, `newest`);
 * it does not compare the answer's `revision` (Q5: a restarted core counts from the start again). While an answer is on its way the last rows stay; an empty box asks
 * nothing and clears them at once, and so does a refusal of the newest request.
 */
export function useSearch(focusList: () => void) {
  const [query, setQueryState] = useState("");
  const [active, setActive] = useState(0);
  const [answer, setAnswer] = useState<Answer | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const toTree = useRef(false);
  const searching = query.trim() !== "";
  const rows: SearchNode[] = searching ? (answer?.result.nodes ?? []) : [];
  const docs: SearchDoc[] = searching ? (answer?.result.docs ?? []) : [];
  /** the node rows and then the doc rows are one list for the keys */
  const count = rows.length + docs.length;

  /** The numbering of the requests: an answer to an older one is dropped, and so is one on its way when the box empties. */
  const [numbering] = useState(newest);
  /** When the box last changed. */
  const changedAt = useRef(0);
  const latestQuery = useRef(query);
  latestQuery.current = query;

  const ask = (text: string) => {
    const number = numbering.next();
    const sentAt = performance.now();
    const started = changedAt.current;
    request("search", { query: text }).then(
      (result) => {
        if (outcomeOf(numbering, number, true) !== "draw") return;
        setAnswer({ query: text, result, changedAt: started, sentAt });
        setActive((a) => Math.min(a, Math.max(result.nodes.length + result.docs.length - 1, 0)));
      },
      () => {
        // the core said no (it is not running): if this was the newest request there are no rows, not the last query's; the next change asks again
        if (outcomeOf(numbering, number, false) === "clear") setAnswer(null);
      },
    );
  };
  const askLatest = useRef(ask);
  askLatest.current = ask;

  const setQuery = (value: string) => {
    changedAt.current = performance.now();
    setQueryState(value);
    setActive(0);
  };

  // each change of the box asks the core at once; an empty box asks nothing and drops what was on its way
  useEffect(() => {
    if (!searching) {
      numbering.drop();
      setAnswer(null);
      return;
    }
    askLatest.current(query);
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

  // `focusSearch` is *V-NOC: Search…*: the cursor goes into the box, with the text it was given in it. `setSearch` and `searchKey` are for the test suite, which cannot type or
  // press keys in a webview: the text a person's typing would leave in the box, and a key pressed in it. They reach the latest `handleKey` through a ref, not the one of the render that subscribed.
  const keyLatest = useRef<(name: string) => boolean>(() => false);
  useEffect(
    () =>
      on("viewCommand", ({ name, text, key }) => {
        if (name === "setSearch") setQuery(text ?? "");
        else if (name === "searchKey" && key) keyLatest.current(key);
        else if (name === "focusSearch") {
          if (text !== undefined && text !== "") flushSync(() => setQuery(text)); // drawn now, so the box holds the text when it is asked what it holds
          input.current?.focus();
          if (text === undefined || text === "") input.current?.select();
          post({ type: "searchFocused", focused: input.current !== null && document.activeElement === input.current, text: input.current?.value ?? "" });
        }
      }),
    [],
  );

  // an answer was drawn (this runs after the commit that drew it): the test suite's note
  useEffect(() => {
    if (!answer) return;
    const now = performance.now();
    post({ type: "searchDrawn", query: answer.query, ids: answer.result.nodes.map((n) => n.id), docs: answer.result.docs.map((d) => d.doc_path), total: answer.result.nodes_total, notes: answer.result.notes, ms: now - answer.changedAt, trip: now - answer.sentAt });
  }, [answer]);

  // the canvas dims what the box's text does not find: the text crosses in the session, `null` when the box is empty (05 §3). No debounce, as the rows
  const toldSearch = useRef<string | null>(null);
  useEffect(() => {
    const text = searching ? query : null;
    if (text === toldSearch.current) return;
    toldSearch.current = text;
    dispatch({ type: "search", query: text });
  }, [searching, query]);
  useEffect(
    () => () => {
      if (toldSearch.current !== null) dispatch({ type: "search", query: null });
    },
    [],
  );

  // the active row's card is pointed at on the canvas (a doc row: its node, when it is in the tree); no query, nothing pointed at
  const told = useRef<string | null>(null);
  useEffect(() => {
    const id = searching ? pointedAt(rowAt(rows, docs, active)) : null;
    if (id === told.current) return;
    told.current = id;
    dispatch({ type: "highlight", id });
  }, [searching, rows, docs, active]);
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
  /** The click rule of 07 §4 for a pick (see `rowClick`): the canvas selects a call card it draws for the node, else the node is revealed (the focus shortens if it hides it, 03 §3). The box clears; the list comes back with the row in view. */
  const pick = (row: SearchNode) => {
    const context = useSession.getState().active;
    // nothing is dispatched here: with the canvas open it decides what a pick is (a call card it draws, or a reveal by structure); the host asks it, or reveals for the sidebar
    void request("rowClick", { context, id: row.id, pick: true }).catch(() => {});
    clear();
  };
  /**
   * A doc row: the node is revealed and clicked as a node pick does (when it is in the tree), then the host shows the doc: the docs tab pinned to the node with this doc chosen, or, for
   * a doc of a node that is not in the tree, the `.md` in an editor tab (05 §1).
   */
  const pickDoc = (doc: SearchDoc) => {
    const ask = () => request("showDoc", { node: doc.node, nodeId: doc.node_id, missing: doc.missing, path: doc.doc_path }).catch(() => {});
    if (doc.node === null) void ask();
    else {
      const context = useSession.getState().active;
      void request("rowClick", { context, id: doc.node, pick: true }).then(ask, ask);
    }
    clear();
  };
  /** Picks the row at this place of the list, a node's or a doc's. */
  const pickAt = (at: number) => {
    const row = rowAt(rows, docs, at);
    if (row?.type === "node") pick(row.row);
    else if (row?.type === "doc") pickDoc(row.row);
  };
  /** What a key does in the box (`searchKey`): true when it was one of the box's keys, so the caller keeps it from the box's own typing. */
  const handleKey = (name: string): boolean => {
    const key = searchKey(name, count, active);
    if (!key) return false;
    if (key.type === "move") setActive(key.to);
    else if (key.type === "pick") pickAt(active);
    else clear();
    return true;
  };
  keyLatest.current = handleKey;
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    if (handleKey(e.key)) e.preventDefault();
  };
  /** `/` and `Ctrl/Cmd+F` from anywhere in the view: the cursor goes into the box. */
  const focusBox = () => {
    input.current?.focus();
    input.current?.select();
  };
  /** The answer for the box's text came and found nothing (not: no answer yet). */
  const noMatches = searching && answer?.query === query && count === 0;
  return { query, setQuery, rows, docs, count, total: answer?.result.nodes_total ?? 0, docsTotal: answer?.result.docs_total ?? 0, notes: searching ? (answer?.result.notes ?? []) : [], noMatches, active, setActive, searching, input, onKeyDown, pick, pickAt, clear, focusBox };
}
