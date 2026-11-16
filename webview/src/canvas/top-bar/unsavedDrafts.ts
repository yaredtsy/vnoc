// The bar's "N unsaved drafts" (docs/17-edit-in-canvas.md §7, 9.5a; the chips' look is docs/16-ui-redesign/02-top-bar.md). Pure, so the rules are tested; the chip only draws.

import { type Drafts, isUnsaved } from "../../state/draftRules.ts";

export interface UnsavedDrafts {
  count: number;
  /** "1 unsaved draft", "3 unsaved drafts" */
  label: string;
  /** the ids of the cards that hold them, in reading order: by file, then by line, then by key. A click walks this list */
  cards: string[];
}

/** The chip's content, or null (no chip) when no draft is unsaved. */
export function unsavedDrafts(drafts: Drafts): UnsavedDrafts | null {
  const held = Object.values(drafts)
    .filter(isUnsaved)
    .sort((a, b) => a.path.localeCompare(b.path) || a.range.start.line - b.range.start.line || a.range.start.character - b.range.start.character || a.key.localeCompare(b.key));
  if (held.length === 0) return null;
  return { count: held.length, label: held.length === 1 ? "1 unsaved draft" : `${held.length} unsaved drafts`, cards: held.map((d) => d.card) };
}

/** The card a click goes to: the one after `last` in the list, back to the first after the end; the first when `last` is not in the list (never clicked, or that draft is gone). */
export function nextCard(cards: readonly string[], last: string | null): string | null {
  if (cards.length === 0) return null;
  const at = last === null ? -1 : cards.indexOf(last);
  return cards[(at + 1) % cards.length];
}
