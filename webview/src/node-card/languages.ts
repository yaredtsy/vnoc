// Which colours a file's code gets: its extension to the Monaco language id. Apart from monaco.ts so a test can load it (Monaco does not load under node).

/**
 * File extension to the Monaco language id (the same ids Shiki registers). Anything else shows as plain text, on purpose: a language the core reads and this table
 * does not know is drawn without colours, never wrongly and never a bug. A line here is a choice of looks, not something a language needs (docs/23 §6).
 * `bend` is drawn with its own grammar, `bendGrammar.ts`: Python's without the marks Python puts on `&&`, `||` and `1n` (it calls them errors), and with `law` as a keyword. Bend 2
 * has no grammar of its own, and Bend's own docs fence Bend code as python (docs/25-bend-film README, "Colour").
 */
const LANGUAGES: Record<string, string> = {
  py: "python", pyi: "python",
  bend: "bend",
  rs: "rust",
  ts: "typescript", tsx: "tsx", js: "javascript", jsx: "jsx", mjs: "javascript", cjs: "javascript",
  json: "json",
  md: "markdown",
};

export function languageOf(path: string): string {
  return LANGUAGES[path.slice(path.lastIndexOf(".") + 1).toLowerCase()] ?? "plaintext";
}
