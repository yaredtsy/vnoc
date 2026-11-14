// What a gesture on the STRUCTURE view does (docs/16-ui-redesign/06-sidebar.md §4, 07 §4). One panel in part 1, MAIN's: whatever the user does in it makes
// MAIN the active context first (a click anywhere in a panel activates it and does what the click does), so the canvas follows.

import { type Intent, MAIN } from "@vnoc/shared/session.ts";
import { request } from "../host.ts";
import { dispatch, useSession } from "../state/session.ts";

/** Makes MAIN the active context when another one is (the user was exploring a call on the canvas). */
export function activateMain(): void {
  if (useSession.getState().active !== MAIN) dispatch({ type: "activate", id: MAIN }, MAIN);
}

/** An intent in MAIN, after activating it. */
export function act(intent: Intent): void {
  activateMain();
  dispatch(intent, MAIN);
}

/** A click on a row: select it, then the host applies the click rule (the canvas is open → reveal there; closed → the code as a preview tab). */
export function clickRow(id: string): void {
  act({ type: "select", id });
  void request("rowClick", { context: MAIN, id }).catch(() => {});
}
