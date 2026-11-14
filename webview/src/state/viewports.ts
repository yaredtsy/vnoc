// The one copy of each context's pan and zoom (canvas/viewports.ts has the rules). In the page only: not saved, so a new window fits every context.

import { type Viewport, type Viewports, forgetClosed, moved, viewportFor } from "../canvas/viewports.ts";
import { useSession } from "./session.ts";

let held: Viewports = {};

/** The canvas calls this as it is panned or zoomed. */
export function rememberViewport(context: string, at: string, viewport: Viewport): void {
  held = moved(held, context, at, viewport);
}

/** The pan and zoom to start `context`'s canvas with; null: fit the view. */
export const savedViewport = (context: string, at: string): Viewport | null => viewportFor(held, context, at);

// a closed context is forgotten, so one opened later for the same call starts fitted
useSession.subscribe((s) => {
  held = forgetClosed(held, s.order);
});
