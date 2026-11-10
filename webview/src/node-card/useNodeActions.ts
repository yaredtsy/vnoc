import { useMemo } from "react";

import { request } from "../host.ts";
import { dispatch } from "../state/session.ts";
import { setDocsOpen, toggleCode } from "../state/view.ts";

/** What a card's buttons do. Each one is a session intent, a request to the host, or a change of this webview's own view. */
export function useNodeActions(id: string, contextId: string) {
  return useMemo(() => {
    const showDocs = () => {
      dispatch({ type: "select", id }, contextId);
      setDocsOpen(true);
    };
    return {
      select: () => dispatch({ type: "select", id }, contextId),
      focus: () => dispatch({ type: "focus", id }, contextId),
      toggle: () => dispatch({ type: "toggle", id }, contextId),
      toggleCode: () => toggleCode(id),
      showDocs,
      goToCode: () => void request("goToCode", { key: id }).catch(() => {}),
      // the host asks for the title, attaches, and opens the doc; if it fails it says so itself. When a doc was added, the panel shows it.
      addDoc: () => void request("addDoc", { key: id }).then((r) => r.added && showDocs(), () => {}),
    };
  }, [id, contextId]);
}
