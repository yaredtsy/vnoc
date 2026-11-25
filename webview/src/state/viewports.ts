// The one copy of each context's pan and zoom (canvas/viewports.ts has the rules). In the page only: not saved, so a new window fits every context.

import { type Viewport, type Viewports, forgetClosed, moved, viewportFor } from "../canvas/viewports.ts";
import { post } from "../host.ts";
import { useSession } from "./session.ts";

let held: Viewports = {};

/** The canvas calls this as it is panned or zoomed. */
export function rememberViewport(context: string, at: string, viewport: Viewport): void {
  held = moved(held, context, at, viewport);
  post({ type: "viewport", context, x: viewport.x, y: viewport.y, zoom: viewport.zoom }); // the extension's test suite reads it
}

/** The pan and zoom to start `context`'s canvas with; null: fit the view. */
export const savedViewport = (context: string, at: string): Viewport | null => viewportFor(held, context, at);

// a closed context is forgotten, so one opened later for the same call starts fitted
useSession.subscribe((s) => {
  held = forgetClosed(held, s.order);
});

/** Tells the host the viewport a canvas starts with, without remembering anything: the extension's test suite reads it (a context that comes back reports the one it kept). */
export const reportViewport = (context: string, viewport: Viewport): void => post({ type: "viewport", context, x: viewport.x, y: viewport.y, zoom: viewport.zoom });
