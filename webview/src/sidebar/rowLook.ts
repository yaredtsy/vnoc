// How a sidebar row shows its state (docs/16-ui-redesign/09-states.md §3). Each state has a channel that is not colour alone: an outline (solid, or dotted for
// "pointed at"), a glyph, a word. In High Contrast `--vscode-contrastActiveBorder` is defined and the outline takes its colour; in the other themes it is empty
// and the list's own selection colours carry the state, as in every VS Code list. Pure, so the table is a test.

export interface RowState {
  selected: boolean;
  /** pointed at without selecting: a hovered crumb or card, a search result (`session.highlight`) */
  highlighted: boolean;
  /** the list has the keyboard */
  treeFocused: boolean;
}

const OUTLINE = "outline outline-1 -outline-offset-1";

/**
 * The classes of a row's element. Selected with the keyboard in the list: the list's active colours and its focus outline (or the High Contrast border);
 * selected without it: the dim colours and, in High Contrast, the border. Highlighted: the hover colour and a **dotted** outline, which is also how a card
 * is pointed at. Neither: the hover colour under the pointer.
 */
export function rowClasses(s: RowState): string {
  const base = "group absolute inset-x-0 flex cursor-pointer items-center pr-2 select-none";
  if (s.selected && s.treeFocused) return `${base} bg-[var(--card-selected-bg)] text-[color:var(--list-active-fg)] [--row-bg:var(--card-selected-bg)] ${OUTLINE} outline-[color:var(--row-outline-focused)]`;
  if (s.selected) return `${base} bg-[var(--list-inactive-bg)] text-[color:var(--list-inactive-fg)] [--row-bg:var(--list-inactive-bg)] ${OUTLINE} outline-[color:var(--row-outline-selected)]`;
  if (s.highlighted) return `${base} bg-[var(--list-hover-bg)] [--row-bg:var(--list-hover-bg)] ${OUTLINE} outline-dotted outline-[color:var(--card-highlight)]`;
  return `${base} hover:bg-[var(--list-hover-bg)] hover:[--row-bg:var(--list-hover-bg)]`;
}

/** A row whose file did not parse has its name in the error colour; `syntax error` is its description, so the colour is not the only word. */
export const nameClasses = (syntaxError: boolean): string => (syntaxError ? "min-w-0 truncate text-destructive" : "min-w-0 truncate");
