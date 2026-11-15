// Each context's pan and zoom, remembered in the page while it is open (docs/16-ui-redesign/03-canvas.md §4). Pure; `state/viewports.ts` holds the one copy.
// Not saved across windows. The canvas keeps `moved` up to date as it is panned, so leaving a context has already saved it.

import type { ContextId } from "@vnoc/shared/session.ts";

export interface Viewport {
  x: number;
  y: number;
  zoom: number;
}

/** A viewport is the pan and zoom of one drawing: `at` names it (the same drawn root, the same load of the structure), so a context that was refocused is fitted again. */
export type Viewports = Readonly<Record<ContextId, { at: string; viewport: Viewport }>>;

/** Remember the viewport `context` has now. The same object when it is the one already held. */
export function moved(held: Viewports, context: ContextId, at: string, viewport: Viewport): Viewports {
  const old = held[context];
  if (old && old.at === at && old.viewport.x === viewport.x && old.viewport.y === viewport.y && old.viewport.zoom === viewport.zoom) return held;
  return { ...held, [context]: { at, viewport } };
}

/** What to show when `context` comes up: its own pan and zoom, or null to fit the view (never seen, or the drawing is another one now). */
export function viewportFor(held: Viewports, context: ContextId, at: string): Viewport | null {
  const mine = held[context];
  return mine && mine.at === at ? mine.viewport : null;
}

/** Forget the contexts that are closed. The same object when none was. */
export function forgetClosed(held: Viewports, open: readonly ContextId[]): Viewports {
  const gone = Object.keys(held).filter((id) => !open.includes(id));
  if (gone.length === 0) return held;
  return Object.fromEntries(Object.entries(held).filter(([id]) => open.includes(id)));
}
