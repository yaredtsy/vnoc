// The `ID:` line of a docstring, as the core reads it (docs/17-edit-in-canvas.md §7). A port, not a new rule: the reader is `ids::read` of the `python-syntax` crate
// (language_driver_rs, `languages/python/crates/python-syntax/src/ids.rs`), which the Python driver uses for every `ID:` it reports. It looks at every `:`, takes the
// word before it and the word after it, and counts the pair when the first word is `ID` and neither word holds another `:`. It is deliberately loose: prose that says
// `ID: something` counts, as it does for the core, so a line the core would read as the ID is a line this reads as one. Pure; indentation never matters (words are
// split on whitespace), so it reads `base` and the re-indented text alike. **One adaptation:** the core reads the docstring's content, and this reads the function's source, where
// a docstring's quotes are glued to the words (`"""Summary. ID: X"""`): an opening quote (with its string prefix) is taken off the word before the colon and closing quotes off the
// word after it. An ID never holds a quote (the core refuses to write one that does).
//
// **Any language:** the marker of a def can be a line comment (`# ID: x` above a Bend def, `// ID: x` in a language with those) as well as a line of a docstring. The word before the
// colon loses a leading comment mark (`#`, `//`, `///`, `/*`, `*`), so `#ID: x` is `ID: x`; a mark that stands alone (`# ID: x`) is a word of its own and never touched the label. Python's
// quote rule is unchanged. The reader stays loose on purpose: a def whose doc comment says "ID: x" in prose counts, as it does for Python and for the core.

const LABEL = "ID";
const SPACE = /\s/;
/** A comment mark glued to the label: `#ID:`, `//ID:`, `///ID:`, `/*ID:`, `*ID:` count as `ID:`, so a line comment in any language is read as the label is. */
const COMMENT_MARK = /^(?:\/\/\/|\/\/|\/\*|#|\*)+/;
const OPENING = /^(?:[rRuUbBfF]{1,2})?["']+/;
const CLOSING = /["']+$/;

/** The word that ends just before `end`, skipping the whitespace in front of it. */
function wordBefore(text: string, end: number): string {
  let to = end;
  while (to > 0 && SPACE.test(text[to - 1])) to--;
  let from = to;
  while (from > 0 && !SPACE.test(text[from - 1])) from--;
  return text.slice(from, to);
}

/** The word that starts at or after `start`, skipping the whitespace in front of it. */
function wordAfter(text: string, start: number): string {
  let from = start;
  while (from < text.length && SPACE.test(text[from])) from++;
  let to = from;
  while (to < text.length && !SPACE.test(text[to])) to++;
  return text.slice(from, to);
}

/** Every value written as `ID: <value>` in `text`, in order. (The core keeps the last one; a function with two lines has two.) */
export function idValues(text: string): string[] {
  const found: string[] = [];
  for (let i = text.indexOf(":"); i !== -1; i = text.indexOf(":", i + 1)) {
    const key = wordBefore(text, i).replace(OPENING, "").replace(COMMENT_MARK, "");
    const value = wordAfter(text, i + 1).replace(CLOSING, "");
    if (key === LABEL && value !== "" && !value.includes(":")) found.push(value);
  }
  return found;
}

/**
 * Whether saving `written` over `base` takes an ID away: `base` has an `ID:` value that `written` does not have anywhere. An ID removed, an ID changed (the old value is
 * gone) and an ID that was never there (nothing to lose) are the three answers; an ID that only moved to another line, or was kept, is not a loss. A class's text holds its
 * methods' lines too, so an ID of a method taken away counts.
 */
export function dropsId(base: string, written: string): boolean {
  const kept = new Set(idValues(written));
  return idValues(base).some((id) => !kept.has(id));
}
