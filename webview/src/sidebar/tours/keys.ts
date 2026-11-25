// The keys of the TOURS section (docs/26-tours/design/07-tour-list.md §8, 04 §1.3). Pure: what a key does to the row focus, and what the section does with it.

export type Move = { type: "focus"; index: number } | { type: "open" } | { type: "list" } | null;

/**
 * `↑` `↓` move the row focus (never past the first or the last row), `Home` and `End` go to the ends, `Enter` and `Space` open the row (the list) or jump to it
 * (the route), `Escape` in the route goes back to the list. `length` is how many rows there are, `index` the one that has the focus (-1: none yet).
 */
export function tourKey(key: string, index: number, length: number, mode: "list" | "route"): Move {
  if (length === 0) return key === "Escape" && mode === "route" ? { type: "list" } : null;
  switch (key) {
    case "ArrowDown":
      return { type: "focus", index: Math.min(length - 1, index + 1) };
    case "ArrowUp":
      return { type: "focus", index: Math.max(0, index <= 0 ? 0 : index - 1) };
    case "Home":
      return { type: "focus", index: 0 };
    case "End":
      return { type: "focus", index: length - 1 };
    case "Enter":
    case " ":
      return index >= 0 ? { type: "open" } : null;
    case "Escape":
      return mode === "route" ? { type: "list" } : null;
    default:
      return null;
  }
}
