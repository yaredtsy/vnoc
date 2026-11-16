// The card shows a definition's text with its own indentation taken off, and puts it back on save (docs/17-edit-in-canvas.md §6, "Putting the indent back").
// The range of a definition starts at `def`, not at the start of its line: the first line has no indent in the text, the others carry the line's `prefix`
// (the first line's text before `range.start`, tabs or spaces as written). Pure, and shared: the card shows with `dedent`, the host writes with `reindent`.

/** A line of text, without its terminator: `\n` and `\r\n` both end one. */
export const splitLines = (text: string): string[] => text.split(/\r?\n/);

/** One line, minus the prefix's length of leading whitespace, or all of it when it has less (it takes off only what is there). */
const dedentLine = (line: string, prefix: string): string => line.slice(Math.min(prefix.length, line.length - line.trimStart().length));

/** For display. The first line stays. Every other line loses the prefix's length of leading whitespace, or all of it when it has less. */
export function dedent(text: string, prefix: string): string {
  const [first, ...rest] = text.split("\n");
  return [first, ...rest.map((l) => dedentLine(l, prefix))].join("\n");
}

/** Until the host says what the first line's text before `range.start` is (`editStart`), the card only knows how many columns it is. */
export const readingPrefix = (column: number): string => " ".repeat(column);

/** More cells than this and the lines between the common head and tail are not matched: a rewrite that big is written as new text. */
const MAX_DIFF_CELLS = 4_000_000;

/**
 * For each line of `a`, the index of the line of `b` it is matched to, or -1: the longest common subsequence, after the lines the two share at the start and at
 * the end are matched without looking (an edit in the middle of a big class costs the size of the edit, not of the class).
 */
function matchLines(a: readonly string[], b: readonly string[]): number[] {
  const match = new Array<number>(a.length).fill(-1);
  let head = 0;
  while (head < a.length && head < b.length && a[head] === b[head]) match[head] = head, head++;
  let tail = 0;
  while (tail < a.length - head && tail < b.length - head && a[a.length - 1 - tail] === b[b.length - 1 - tail]) match[a.length - 1 - tail] = b.length - 1 - tail, tail++;
  const n = a.length - head - tail;
  const m = b.length - head - tail;
  if (n === 0 || m === 0 || n * m > MAX_DIFF_CELLS) return match;
  // lcs[i][j]: the length of the longest common subsequence of a[head + i ..] and b[head + j ..]
  const width = m + 1;
  const lcs = new Uint32Array((n + 1) * width);
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i * width + j] = a[head + i] === b[head + j] ? lcs[(i + 1) * width + j + 1] + 1 : Math.max(lcs[(i + 1) * width + j], lcs[i * width + j + 1]);
    }
  }
  for (let i = 0, j = 0; i < n && j < m; ) {
    if (a[head + i] === b[head + j]) match[head + i++] = head + j++;
    else if (lcs[(i + 1) * width + j] >= lcs[i * width + j + 1]) i++;
    else j++;
  }
  return match;
}

/**
 * The text to write into the range for a draft. `draft` is what the card holds (the lines dedented); `base` is the range's text as the host read it when the
 * draft started, `prefix` the first line's text before the range, `eol` the document's line ending.
 * - A line of the draft that is a line of `dedent(base)` (matched by a line diff) is written as the base's own line, byte for byte: its indent, its trailing
 *   whitespace, a docstring line at the margin. So a draft that was not changed gives back the same bytes, whatever the text.
 * - A line the user added or changed gets `prefix` in front, and an empty one stays empty. The first line goes in as it is: the range starts after `prefix`.
 * - Lines are split on `\n` and `\r\n` and joined with `eol`. The one thing not given back is a line ending that differs from the document's.
 * The leftover: an edited line that sat at the margin, or had less indent than `prefix`, comes back with the whole prefix.
 */
export function reindent(draft: string, base: string, prefix: string, eol: string): string {
  const lines = splitLines(draft);
  const original = splitLines(base);
  const shown = original.map((l, i) => (i === 0 ? l : dedentLine(l, prefix)));
  // the first line is matched with the first line only: the base's own first line has no indent, and put anywhere else it would be in the wrong column
  const rest = matchLines(lines.slice(1), shown.slice(1));
  const out = lines.map((line, i) => {
    if (i === 0) return line;
    const at = rest[i - 1];
    if (at >= 0) return original[at + 1];
    return line === "" ? line : prefix + line;
  });
  return out.join(eol);
}
