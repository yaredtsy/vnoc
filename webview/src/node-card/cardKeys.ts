// Which keys a card swallows (docs/17-edit-in-canvas.md §7, measured in step 9.0 check A). A webview forwards every key to VS Code, `defaultPrevented` or not,
// so Monaco's own ⌘S would also run VS Code's Save: `stopPropagation()` on the card's container is what stops it. Pure, so the table is tested; the card's
// listener only builds a `KeyPress` from the event and calls `stopPropagation()` when this says so.

import type { EditState } from "./editState.ts";

export type Platform = "mac" | "other";

/** The parts of a `KeyboardEvent` the rule reads. */
export interface KeyPress {
  key: string;
  code: string;
  meta: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
}

/**
 * While the card is `editing`, and in no other state:
 * - macOS: ⌘S, ⌘⌥S (Save All: it saves the draft only) and ⌘⇧S (Save As: nothing). Ctrl+S is not Save there.
 * - elsewhere: Ctrl+S and Ctrl+Shift+S. Ctrl+K S (Save All) is a chord whose first key must reach VS Code, so it is never swallowed: its S is a plain `s`.
 * Every other key passes: ⌘P, ⌘W, ⌘B, ⌘C, a plain `s`…
 * The S is `key`, or `code` when ⌥ is held (macOS gives ⌥S the key `ß`).
 */
export function swallows(state: EditState, k: KeyPress, platform: Platform): boolean {
  if (state !== "editing") return false;
  const isS = k.key.toLowerCase() === "s" || (k.alt && k.code === "KeyS");
  if (!isS) return false;
  if (platform === "mac") return k.meta && !k.ctrl && (k.alt ? !k.shift : true); // ⌘S, ⌘⇧S, ⌘⌥S; not ⌘⌥⇧S
  return k.ctrl && !k.meta && !k.alt; // Ctrl+S, Ctrl+Shift+S; Ctrl+Alt is AltGr on some layouts
}

/** The Save button's tooltip: it names the key. */
export const saveTip = (platform: Platform): string => (platform === "mac" ? "Save (⌘S)" : "Save (Ctrl+S)");

/** Not pure (it reads the page), so it stays out of the tests: the rules above take the platform as an argument. */
export function platformNow(): Platform {
  return /Mac/i.test(navigator.platform || navigator.userAgent) ? "mac" : "other";
}
