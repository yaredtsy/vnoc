// The context chips on the bar (docs/16-ui-redesign/02-top-bar.md §3 #2, §5). Pure, so the rules are tested; ContextChips only draws them.

import { type CallFacts, type CallId, linesText } from "@vnoc/shared/calls.ts";
import type { ContextId, ContextState, Session } from "@vnoc/shared/session.ts";

/** Below this width the chips become one chip with the active context's name and a menu. */
export const ONE_CHIP = 360;
/** How many EXPLORE chips are drawn; the oldest of the rest fold into `+N ▾`. */
export const MAX_EXPLORE = 3;

export interface ContextChip {
  id: ContextId;
  label: string;
  tip: string;
  active: boolean;
  /** an EXPLORE chip and a tour's chip have a ✕; MAIN stays */
  closable: boolean;
  /** `tour`: `▶ title` (docs/26-tours/design/04-route-and-map.md §5 #2); it never folds into `+N` */
  kind: ContextState["kind"];
}

export type Chips =
  /** one context: no chips */
  | null
  | { mode: "row"; shown: ContextChip[]; folded: ContextChip[] }
  /** narrow: the active one, and a menu of them all */
  | { mode: "one"; current: ContextChip; all: ContextChip[] };

/**
 * MAIN and one chip per EXPLORE context, in `session.order`, only with two contexts or more. More than MAX_EXPLORE EXPLORE chips: the earliest in the
 * order fold, except the active one, which is never hidden. Under ONE_CHIP: one chip, the active one's. `facts` says what a call id is, for the tooltip.
 */
export function chipsFor(session: Pick<Session, "contexts" | "order" | "active">, width: number, facts: (call: CallId) => CallFacts | null): Chips {
  if (session.order.length < 2) return null;
  const chips = session.order.map<ContextChip>((id) => {
    const c = session.contexts[id];
    const found = c.call === null ? null : facts(c.call);
    const tip = c.kind === "main" ? "Main" : c.kind === "tour" ? `Tour of ${c.title}` : found ? `Explore: ${c.title}, from ${found.callerName} (${linesText(found.lines)})` : `Explore: ${c.title}`;
    return { id, label: c.kind === "tour" ? `▶ ${c.title}` : c.title, tip, active: id === session.active, closable: c.kind !== "main", kind: c.kind };
  });
  if (width < ONE_CHIP) return { mode: "one", current: chips.find((c) => c.active) ?? chips[0], all: chips };
  const explore = chips.filter((c) => c.kind === "explore");
  let toFold = Math.max(0, explore.length - MAX_EXPLORE);
  const folded = new Set<ContextId>();
  for (const c of explore) {
    if (toFold === 0) break;
    if (c.active) continue;
    folded.add(c.id);
    toFold--;
  }
  return { mode: "row", shown: chips.filter((c) => !folded.has(c.id)), folded: chips.filter((c) => folded.has(c.id)) };
}
