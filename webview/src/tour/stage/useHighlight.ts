import { useEffect, useState } from "react";
import type { HighlighterCore, ThemedToken } from "shiki/core";

import { currentTheme, fenceLanguage, loadHighlighter } from "../../lib/highlighter.ts";
import type { Highlight } from "../text.ts";

/** The theme VS Code has now (`github-light` or `github-dark`): it follows the body's class, which VS Code changes when the colour theme does. */
export function useThemeName(): string {
  const [name, setName] = useState(currentTheme);
  useEffect(() => {
    const watch = new MutationObserver(() => setName(currentTheme()));
    watch.observe(document.body, { attributes: true, attributeFilter: ["class"] });
    return () => watch.disconnect();
  }, []);
  return name;
}

/** The cards' highlighter, once it is loaded (null before; the stage draws plain until then and is drawn again with the colours). The same instance Monaco gets. */
export function useHighlighter(): HighlighterCore | null {
  const [hl, setHl] = useState<HighlighterCore | null>(null);
  useEffect(() => {
    let live = true;
    loadHighlighter().then(
      (h) => live && setHl(h),
      () => {}, // no colours is plain text, never a broken stage
    );
    return () => {
      live = false;
    };
  }, []);
  return hl;
}

/** What the text renderer needs of a highlighter: the callback and the languages (`text.ts` does not import Shiki). */
export function highlightEnv(hl: HighlighterCore | null, theme: string): { highlight?: Highlight; languages?: readonly string[]; fenceLanguage?: typeof fenceLanguage } {
  if (!hl) return {};
  return {
    languages: hl.getLoadedLanguages(),
    fenceLanguage,
    highlight: (code, lang) => {
      try {
        return hl.codeToHtml(code.replace(/\n$/, ""), { lang, theme });
      } catch {
        return null;
      }
    },
  };
}

/** The tokens of a text, line by line, as `lines` has them; null while the highlighter loads, and for a language it does not have (plain). */
export function useTokens(hl: HighlighterCore | null, text: string | null, language: string, theme: string): ThemedToken[][] | null {
  const [tokens, setTokens] = useState<ThemedToken[][] | null>(null);
  useEffect(() => {
    if (!hl || text === null || !hl.getLoadedLanguages().includes(language)) return setTokens(null);
    try {
      setTokens(hl.codeToTokensBase(text, { lang: language, theme }));
    } catch {
      setTokens(null);
    }
  }, [hl, text, language, theme]);
  return tokens;
}
