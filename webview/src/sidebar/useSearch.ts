import { useEffect, useMemo, useRef, useState } from "react";

import type { TreeNode } from "@vnoc/shared/messages.ts";
import { request } from "../host.ts";
import { dispatch, useSession } from "../state/session.ts";
import { type Result, indexNodes, pickIntent, rankNodes, searchKey } from "./search.ts";

/**
 * The search box's state and what it does (06 §5): the query, the results, the active result. While there is a query the panels give way to the results;
 * the active result is pointed at on the canvas (`highlight`: nothing is selected); `Enter` or a click picks it; `Esc` clears. `focusList` gives the
 * keyboard to the active panel's list when the search ends. Keeps no state a gesture could need elsewhere: the session has the rest.
 */
export function useSearch(root: TreeNode | null, focusList: () => void) {
  const [query, setQueryState] = useState("");
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const toTree = useRef(false);
  const index = useMemo(() => (root ? indexNodes(root) : []), [root]);
  const results = useMemo<Result[]>(() => rankNodes(index, query), [index, query]);
  const searching = query.trim() !== "";

  const setQuery = (value: string) => {
    setQueryState(value);
    setActive(0);
  };

  // the active result's card is pointed at on the canvas; no query, nothing pointed at
  const told = useRef<string | null>(null);
  useEffect(() => {
    const id = searching ? (results[active]?.node.id ?? null) : null;
    if (id === told.current) return;
    told.current = id;
    dispatch({ type: "highlight", id });
  }, [searching, results, active]);
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
  const pick = (result: Result) => {
    const id = result.node.id;
    const context = useSession.getState().active;
    dispatch(pickIntent(id));
    void request("rowClick", { context, id }).catch(() => {});
    clear();
  };
  const onKeyDown = (e: React.KeyboardEvent) => {
    const key = searchKey(e.key, results.length, active);
    if (!key || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    e.preventDefault();
    if (key.type === "move") setActive(key.to);
    else if (key.type === "pick") pick(results[active]);
    else clear();
  };
  /** `/` and `Ctrl/Cmd+F` from anywhere in the view: the cursor goes into the box. */
  const focusBox = () => {
    input.current?.focus();
    input.current?.select();
  };
  return { query, setQuery, results, active, setActive, searching, input, onKeyDown, pick, clear, focusBox };
}
