// The webview's copy of the session (docs/10-ui-rework/01-state.md §5). Read-only: the only way to change the session is `dispatch`, which asks the
// extension host and gets the new snapshot back. A webview never sets a field of its copy.

import { useSyncExternalStore } from "react";

import { type ContextId, type Intent, type Session, emptySession, keepEqual } from "@vnoc/shared/session.ts";
import { on, post, request } from "../host.ts";

let current: Session = emptySession();
const listeners = new Set<() => void>();

function accept(incoming: Session) {
  if (incoming.seq < current.seq) return; // a snapshot that arrived late
  current = keepEqual(current, incoming);
  listeners.forEach((listener) => listener());
}

// ask once when the webview starts; after that the host sends every change
request("session").then(accept, () => {});
on("session", accept);

/**
 * A part of the session. The selector must return something that is the same value when nothing it reads changed (a string, a boolean, or an array
 * that `keepEqual` kept), or the component draws again on every snapshot.
 */
export function useSession<T>(select: (s: Session) => T): T {
  const subscribe = (listener: () => void) => (listeners.add(listener), () => void listeners.delete(listener));
  return useSyncExternalStore(subscribe, () => select(current));
}

/** The only way a component changes the session. */
export function dispatch(intent: Intent, context: ContextId = current.active): void {
  post({ type: "intent", context, intent });
}
