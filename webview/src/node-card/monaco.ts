// Monaco with Shiki for the colours. Monaco's own Monarch tokenizers are dropped: Shiki (the TextMate grammars VS Code itself uses)
// tokenizes, and `shikiToMonaco` hands the result to Monaco. Loaded once, on the first card that opens its code.
//
// Webview limits (the CSP in extension.ts): no wasm, so Shiki runs on its JavaScript regex engine; no `worker-src`, so Monaco has no
// language workers and falls back to the main thread (fine: read-only slices, and Shiki does the tokenizing, not a worker).

import * as monaco from "monaco-editor/editor/editor.api.js";
import { shikiToMonaco } from "@shikijs/monaco";
import { createHighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";

const LIGHT = "github-light";
const DARK = "github-dark";

/** File extension to the Monaco language id (the same ids Shiki registers). Anything else shows as plain text. */
const LANGUAGES: Record<string, string> = {
  py: "python", pyi: "python",
  rs: "rust",
  ts: "typescript", tsx: "tsx", js: "javascript", jsx: "jsx", mjs: "javascript", cjs: "javascript",
  json: "json",
};

export function languageOf(path: string): string {
  return LANGUAGES[path.slice(path.lastIndexOf(".") + 1).toLowerCase()] ?? "plaintext";
}

/** VS Code puts `vscode-light` / `vscode-dark` / `vscode-high-contrast(-light)` on `<body>`. */
export function currentTheme(): string {
  const c = document.body.classList;
  return c.contains("vscode-light") || c.contains("vscode-high-contrast-light") ? LIGHT : DARK;
}

let ready: Promise<typeof monaco> | null = null;

export function loadMonaco(): Promise<typeof monaco> {
  return (ready ??= (async () => {
    const highlighter = await createHighlighterCore({
      themes: [import("@shikijs/themes/github-light"), import("@shikijs/themes/github-dark")],
      langs: [
        import("@shikijs/langs/python"),
        import("@shikijs/langs/rust"),
        import("@shikijs/langs/typescript"),
        import("@shikijs/langs/tsx"),
        import("@shikijs/langs/javascript"),
        import("@shikijs/langs/jsx"),
        import("@shikijs/langs/json"),
      ],
      engine: createJavaScriptRegexEngine(),
    });
    // `shikiToMonaco` only tokenizes languages Monaco already knows, and the bare editor API registers none
    for (const id of highlighter.getLoadedLanguages()) monaco.languages.register({ id });
    shikiToMonaco(highlighter, monaco);
    // the user switches VS Code's theme while the canvas is open: follow it (the theme is global to all Monaco editors)
    new MutationObserver(() => monaco.editor.setTheme(currentTheme())).observe(document.body, { attributes: true, attributeFilter: ["class"] });
    return monaco;
  })());
}
