import { useMemo } from "react";

import { request } from "../host.ts";
import { dispatch } from "../state/session.ts";
import { setDocsOpen, toggleCode } from "../state/view.ts";

/**
 * What a card's buttons do. Each one is a session intent, a request to the host, or a change of this webview's own view. `id` is the card's (a structure
 * key or a call id); `key` is the structure node it shows, which is the callee for a call card (Go to Code and the docs are the callee's).
 */
export function useNodeActions(id: string, contextId: string, key: string | null = id) {
  return useMemo(() => {
    const showDocs = () => {
      dispatch({ type: "select", id }, contextId);
      setDocsOpen(true);
    };
    return {
      select: () => dispatch({ type: "select", id }, contextId),
      focus: () => dispatch({ type: "focus", id }, contextId),
      // follow a call: its EXPLORE context opens and is activated
      explore: () => dispatch({ type: "explore", call: id, activate: true }, contextId),
      toggle: () => dispatch({ type: "toggle", id }, contextId),
      toggleCode: () => toggleCode(id),
      showDocs,
      goToCode: () => key !== null && void request("goToCode", { key }).catch(() => {}),
      // the host asks for the title, attaches, and opens the doc; if it fails it says so itself. When a doc was added, the panel shows it.
      addDoc: () => key !== null && void request("addDoc", { key }).then((r) => r.added && showDocs(), () => {}),
    };
  }, [id, contextId, key]);
}
