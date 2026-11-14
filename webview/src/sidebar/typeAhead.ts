// Type-ahead (docs/16-ui-redesign/06-sidebar.md §7): a letter selects the next visible row whose name starts with what was typed in the last 500 ms. Pure; the
// tree keeps the state between keys.

import type { KeyFacts } from "./keys.ts";

/** What was typed, and when the last letter was. */
export interface TypeAhead {
  text: string;
  at: number;
}

export const TYPE_AHEAD_MS = 500;
export const noTypeAhead: TypeAhead = { text: "", at: -Infinity };

/** A letter, digit or mark that is part of a name: one character, no Ctrl, Cmd or Alt, and not Space (open / close) or `/` (the search box). */
export const isTyped = (e: KeyFacts): boolean => e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && e.key !== " " && e.key !== "/";

/**
 * `now` in ms. A key within 500 ms of the last one adds to what was typed; a later one starts again. One character looks from the row after the selected one
 * (so the same letter again goes on to the next row that starts with it); more than one looks from the selected row itself (`ab` stays on `abc` when it
 * starts with it), and the same letter typed several times (`aaa`), which no name starts with, goes on cycling. The search wraps. `id` is the row to select,
 * null when none starts with it (or the key is not a letter): the state is kept, so the next letter can still match.
 */
export function typeAhead(prev: TypeAhead, e: KeyFacts, now: number, rows: readonly { id: string; node: { name: string } }[], selected: string | null): { state: TypeAhead; id: string | null } {
  if (!isTyped(e)) return { state: prev, id: null };
  const text = now - prev.at <= TYPE_AHEAD_MS ? prev.text + e.key : e.key;
  const state = { text, at: now };
  if (rows.length === 0) return { state, id: null };
  const at = selected === null ? -1 : rows.findIndex((r) => r.id === selected);
  const find = (prefix: string, from: number): string | null => {
    const p = prefix.toLowerCase();
    for (let i = 0; i < rows.length; i++) {
      const row = rows[(from + i + rows.length) % rows.length];
      if (row.node.name.toLowerCase().startsWith(p)) return row.id;
    }
    return null;
  };
  if (text.length === 1) return { state, id: find(text, at + 1) };
  const same = [...text.toLowerCase()].every((c) => c === text[0].toLowerCase());
  return { state, id: find(text, Math.max(at, 0)) ?? (same ? find(text[0], at + 1) : null) };
}
