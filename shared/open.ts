// What is open in a context, per focus root (docs/16-ui-redesign/03-canvas.md §2.1). Its own file so `calls.ts` and `session.ts` can both read it.

import type { ContextState } from "./session.ts";

/** How many roots keep an open set, per context: the ones used last. Each set is small; this bounds the saved session. */
export const MAX_ROOTS = 20;

export const NONE: readonly string[] = [];

/** The root of what is drawn: the last step of the focus, "" with none. */
export const rootOf = (focus: readonly string[]): string => (focus.length > 0 ? focus[focus.length - 1] : "");

/** What is open in the context now: the set of its current root. The same array while it does not change, so a component can select it. */
export const openOf = (c: Pick<ContextState, "focus" | "open">): readonly string[] => c.open[rootOf(c.focus)] ?? NONE;
