// What a gesture on a panel of the STRUCTURE view does (docs/16-ui-redesign/06-sidebar.md §4, 07 §4). Whatever the user does in a panel (a click, a key, a
// button, the focus bar) first makes that panel's context the active one when it is not, then does it there: one click, not two. The canvas follows.

import { type ContextId, type Intent } from "@vnoc/shared/session.ts";
import { request } from "../host.ts";
import { dispatch, useSession } from "../state/session.ts";

/** Makes `context` the active one when another is (the user was exploring a call elsewhere). Ends its preview (09 §4): the session does that. */
export function activate(context: ContextId): void {
  if (useSession.getState().active !== context) dispatch({ type: "activate", id: context }, context);
}

/** An intent in a panel's context, after activating it. */
export function act(context: ContextId, intent: Intent): void {
  activate(context);
  dispatch(intent, context);
}

/** A click on a row of a panel: select it there, then the host applies the click rule (the canvas is open → reveal there; closed → the code as a preview tab). */
export function clickRow(context: ContextId, id: string): void {
  act(context, { type: "select", id });
  void request("rowClick", { context, id }).catch(() => {});
}
