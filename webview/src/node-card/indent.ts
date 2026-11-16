// The card shows a definition's text with its own indentation taken off, and puts it back on save (docs/17-edit-in-canvas.md §6, "Putting the indent back").
// The range of a definition starts at `def`, not at the start of its line: the first line has no indent in the text, the others carry the line's `prefix`
// (the first line's text before `range.start`, tabs or spaces as written).

/** The first line stays. Every other line loses the prefix's length of leading whitespace, or all of it when it has less (it takes off only what is there). */
export function dedent(text: string, prefix: string): string {
  const [first, ...rest] = text.split("\n");
  return [first, ...rest.map((l) => l.slice(Math.min(prefix.length, l.length - l.trimStart().length)))].join("\n");
}

/** The first line goes in as it is; every other line that is not empty gets `prefix` back in front. */
export function reindent(text: string, prefix: string): string {
  const [first, ...rest] = text.split("\n");
  return [first, ...rest.map((l) => (l === "" ? l : prefix + l))].join("\n");
}

/** Until the host says what the first line's text before `range.start` is (`editStart`), the card only knows how many columns it is. */
export const readingPrefix = (column: number): string => " ".repeat(column);
