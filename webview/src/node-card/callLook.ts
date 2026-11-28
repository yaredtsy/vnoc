// How a call card looks and what it can do (docs/16-ui-redesign/05-calls-as-nodes.md §6). Pure, so the table is tested; the card only draws it.

import { type Calls, MARK_ORDER, markBadge } from "@vnoc/shared/calls.ts";
import type { CallMark } from "@vnoc/shared/messages.ts";

export interface CallLook {
  /** a chevron to open what the callee calls */
  chevron: boolean;
  /** how many cards opening shows, while it is closed; null when not known (a cut node's callee has not been read) */
  plus: number | null;
  /** the mark drawn in place of the callee's file name: the first of recursive, cut, incomplete, likely; null when none */
  mark: { text: string; title: string } | null;
  /** the project does not know the callee: 60% opacity, no buttons, "not in the index" in place of the file name */
  unknown: boolean;
}

/** What the card knows about the callee's own calls: the children of its node, a cut node's callee tree, a probe, or nothing yet (`Calls`). */
export function callLook(node: { key: string | null; marks: readonly CallMark[]; calls: Calls }, open: boolean): CallLook {
  const known = node.calls.state === "known" ? node.calls.children.length : node.calls.state === "probed" ? node.calls.count : null;
  const cut = node.marks.includes("cut") && (known === null || known === 0);
  const recursive = node.marks.includes("recursive");
  // "calls not loaded" is for a callee whose calls have not arrived: once they are drawn under the card the mark is stale (a callee opened from a cut node shows it no more)
  const mark = MARK_ORDER.find((m) => node.marks.includes(m) && !(m === "cut" && node.calls.state === "known" && known !== null && known > 0));
  // recursive: shown, not entered. cut: the walk stopped here, opening asks for the callee's own tree. Otherwise a chevron when there is something to open
  const chevron = recursive ? false : cut ? true : (known ?? 0) > 0 || node.calls.state === "loading" || node.calls.state === "failed";
  return { chevron, plus: chevron && !open && known !== null && known > 0 ? known : null, mark: mark ? markBadge(mark) : null, unknown: node.key === null };
}
