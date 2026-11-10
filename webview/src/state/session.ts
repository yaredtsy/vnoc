// The webview's copy of the session (docs/10-ui-rework/01-state.md §5). Read-only: the only way to change the session is `dispatch`, which asks the
// extension host and gets the new snapshot back. A webview never sets a field of its copy.

import { create } from "zustand";

import { type ContextId, type Intent, type Session, emptySession, keepEqual } from "@vnoc/shared/session.ts";
import { on, post, request } from "../host.ts";

/**
 * A part of the session, in a component: `useSession((s) => s.contexts[s.active].selected === id)`. A selector returns a primitive, or an array
 * `keepEqual` kept, so a component draws again only when what it reads changed.
 */
export const useSession = create<Session>(() => emptySession());

function accept(incoming: Session) {
  const current = useSession.getState();
  if (incoming.seq < current.seq) return; // a snapshot that arrived late
  useSession.setState(keepEqual(current, incoming), true); // `true`: replace, do not merge
}

// ask once when the webview starts; after that the host sends every change
request("session").then(accept, () => {});
on("session", accept);

/** The only way a component changes the session. */
export function dispatch(intent: Intent, context: ContextId = useSession.getState().active): void {
  post({ type: "intent", context, intent });
}
