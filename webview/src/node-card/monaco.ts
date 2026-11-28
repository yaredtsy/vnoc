// Monaco with Shiki for the colours. Monaco's own Monarch tokenizers are dropped: Shiki (the TextMate grammars VS Code itself uses)
// tokenizes, and `shikiToMonaco` hands the result to Monaco. Loaded once, on the first card that opens its code.
//
// Webview limits (the CSP in extension.ts): no wasm, so Shiki runs on its JavaScript regex engine; no `worker-src`, so Monaco has no
// language workers and falls back to the main thread (fine: read-only slices, and Shiki does the tokenizing, not a worker).

import * as monaco from "monaco-editor/editor/editor.api.js";
import { shikiToMonaco } from "@shikijs/monaco";
import { currentTheme, languageOf, loadHighlighter } from "../lib/highlighter.ts";

// the language table, the theme names and the highlighter itself are `lib/highlighter.ts`: the tour's stage uses them without Monaco
export { currentTheme, languageOf };

let ready: Promise<typeof monaco> | null = null;

export function loadMonaco(): Promise<typeof monaco> {
  return (ready ??= (async () => {
    const highlighter = await loadHighlighter();
    // `shikiToMonaco` only tokenizes languages Monaco already knows, and the bare editor API registers none
    for (const id of highlighter.getLoadedLanguages()) monaco.languages.register({ id });
    shikiToMonaco(highlighter, monaco);
    // the user switches VS Code's theme while the canvas is open: follow it (the theme is global to all Monaco editors)
    new MutationObserver(() => monaco.editor.setTheme(currentTheme())).observe(document.body, { attributes: true, attributeFilter: ["class"] });
    return monaco;
  })());
}
