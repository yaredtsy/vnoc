// The Shiki highlighter the cards' code and the tour's stage share: one instance, the same languages and themes (the TextMate grammars VS Code itself uses). It does not
// need Monaco, so a page that only draws colours (the stage's code pane, a code fence in a step's text) does not load it; `node-card/monaco.ts` hands this same
// highlighter to Monaco. Loaded once, on first use.
//
// Webview limits (the CSP in extension.ts): no wasm, so Shiki runs on its JavaScript regex engine.

import { createHighlighterCore, type HighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";

import { bendGrammar } from "../node-card/bendGrammar.ts";
import { languageOf } from "../node-card/languages.ts";

export const LIGHT = "github-light";
export const DARK = "github-dark";

// the extension to language table is `node-card/languages.ts` (no Monaco in it, so the stage can use it too)
export { languageOf };

/** The names a code fence may call a language by, to the ids above. A name that is neither is plain text. */
const ALIASES: Record<string, string> = { py: "python", python3: "python", rs: "rust", ts: "typescript", js: "javascript", node: "javascript", md: "markdown", jsonc: "json" };

/** The language of a code fence's info string (` ```python `): an id the highlighter has, or null. */
export function fenceLanguage(info: string, loaded: readonly string[]): string | null {
  const name = info.trim().split(/\s+/)[0]?.toLowerCase() ?? "";
  const id = ALIASES[name] ?? name;
  return id !== "" && loaded.includes(id) ? id : null;
}

/** VS Code puts `vscode-light` / `vscode-dark` / `vscode-high-contrast(-light)` on `<body>`. */
export function currentTheme(): string {
  const c = document.body.classList;
  return c.contains("vscode-light") || c.contains("vscode-high-contrast-light") ? LIGHT : DARK;
}

let ready: Promise<HighlighterCore> | null = null;

export function loadHighlighter(): Promise<HighlighterCore> {
  return (ready ??= createHighlighterCore({
    themes: [import("@shikijs/themes/github-light"), import("@shikijs/themes/github-dark")],
    langs: [
      import("@shikijs/langs/python"),
      import("@shikijs/langs/python").then((m) => bendGrammar(m.default[0])), // Bend's: Python's without its error marks
      import("@shikijs/langs/rust"),
      import("@shikijs/langs/typescript"),
      import("@shikijs/langs/tsx"),
      import("@shikijs/langs/javascript"),
      import("@shikijs/langs/jsx"),
      import("@shikijs/langs/json"),
      import("@shikijs/langs/markdown"),
    ],
    engine: createJavaScriptRegexEngine(),
  }));
}
