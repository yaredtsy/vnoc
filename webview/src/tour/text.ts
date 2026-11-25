// The renderer of a step's text (docs/26-tours/05-on-screen.md §7). A step is Markdown, written by a model or pulled with `git pull`: it is untrusted. markdown-it with
// `html: false` (no tag of the text is ever HTML), links go to the host (`data-href`: the click is the page's, nothing navigates), images are their alt text, a code fence
// is highlighted by the cards' Shiki highlighter when it is there (else plain), and a ` ```mermaid ` fence is a placeholder the page fills with a diagram, lazily.
// Everything it returns is the renderer's own output: text is escaped by markdown-it, and the highlighter's HTML is Shiki's, made from escaped tokens. Pure, so it is
// tested without a browser. It is not the docs panel's pipeline: the docs panel has headings and images; a step has neither (the core refuses them).

import MarkdownIt from "markdown-it";

/** The HTML of a text, and the sources of its diagrams in order (a placeholder `data-diagram="n"` stands for `diagrams[n]`). */
export interface Rendered {
  html: string;
  diagrams: string[];
}

/** Highlights `code` as `lang` (an id the highlighter has) into HTML, or null for plain. Given by the page once the highlighter is loaded. */
export type Highlight = (code: string, lang: string) => string | null;

/** What a render is for: the highlighter, and the languages it has (to tell a fence's language from a name it has not got). */
export type Env = {
  diagrams: string[];
  highlight?: Highlight;
  /** the languages the highlighter has loaded; a fence in another is plain */
  languages?: readonly string[];
  /** the name of a fence's language as an id: `py` is `python` */
  fenceLanguage?: (info: string, loaded: readonly string[]) => string | null;
};

const md = new MarkdownIt({ html: false, linkify: true });

// an image is its alt text: a step has none (the core refuses them), but a tour from a pull may
md.renderer.rules.image = (tokens, i) => `<span class="tour-image">${md.utils.escapeHtml(tokens[i].content || "image")}</span>`;

// a link is not followed inside the webview: the click goes to the host. The target travels in `data-href`
md.renderer.rules.link_open = (tokens, i) => `<a href="#" data-href="${md.utils.escapeHtml(String(tokens[i].attrGet("href") ?? ""))}">`;

// a table scrolls sideways inside the pane when it is wide
md.renderer.rules.table_open = () => '<div class="tour-table"><table>';
md.renderer.rules.table_close = () => "</table></div>";

md.renderer.rules.fence = (tokens, i, _options, rawEnv) => {
  const env = rawEnv as Env;
  const token = tokens[i];
  const info = token.info.trim();
  if (info.split(/\s+/)[0]?.toLowerCase() === "mermaid") {
    env.diagrams.push(token.content);
    return `<div class="tour-diagram" data-diagram="${env.diagrams.length - 1}" role="img" aria-label="diagram"><pre class="tour-fence"><code>${md.utils.escapeHtml(token.content)}</code></pre></div>\n`;
  }
  const lang = env.fenceLanguage && env.languages ? env.fenceLanguage(info, env.languages) : null;
  const coloured = lang && env.highlight ? env.highlight(token.content, lang) : null;
  return coloured ? `<div class="tour-fence-box">${coloured}</div>\n` : `<pre class="tour-fence"><code>${md.utils.escapeHtml(token.content)}</code></pre>\n`;
};

/** Renders a step's text. The diagrams' placeholders hold the source as plain code until the page draws them (or when it cannot). */
export function renderText(text: string, env: Partial<Env> = {}): Rendered {
  const full: Env = { diagrams: [], ...env };
  const html = md.render(text, full);
  return { html, diagrams: full.diagrams };
}

/**
 * Draws a diagram: `load` brings Mermaid in (a dynamic import, called only when there is a diagram to draw), `source` is the fence, `id` is unique in the page, `theme` is
 * Mermaid's own (`dark` or `default`). The answer is the SVG Mermaid made (with `securityLevel: "strict"` it is sanitised), or why not: a diagram that does not parse is
 * never a blank or an exception, the page shows its source and the note.
 */
export interface MermaidLike {
  initialize(config: { startOnLoad: boolean; securityLevel: "strict"; theme: string }): void;
  render(id: string, source: string): Promise<{ svg: string }>;
}

export type Drawn = { ok: true; svg: string } | { ok: false; note: string };

export async function drawDiagram(load: () => Promise<MermaidLike>, source: string, id: string, theme: string): Promise<Drawn> {
  try {
    const mermaid = await load();
    mermaid.initialize({ startOnLoad: false, securityLevel: "strict", theme });
    const { svg } = await mermaid.render(id, source);
    return svg.trim() === "" ? { ok: false, note: "This diagram could not be drawn." } : { ok: true, svg };
  } catch {
    return { ok: false, note: "This diagram could not be drawn: the source is shown instead." };
  }
}

/** Mermaid's theme for VS Code's: `dark` on a dark theme, `default` on a light one (the cards choose theirs the same way: `currentTheme` in `lib/highlighter.ts`). */
export const mermaidTheme = (shikiTheme: string): string => (shikiTheme.endsWith("light") ? "default" : "dark");

/** The target of a click on a link of a text: only an `http(s)` address goes to the host; a relative path or anything else goes nowhere. */
export function linkTarget(href: string | null): string | null {
  return href !== null && /^https?:\/\//i.test(href) ? href : null;
}

/** The least a diagram is drawn at, as a share of its natural size: a wide flowchart shrunk to fit the pane was unreadable (TR19b's review). */
export const DIAGRAM_MIN_SHARE = 0.8;

/**
 * The width a diagram is drawn at in a pane `available` px wide: the pane's width, but no less than 80% of its natural width (a wider diagram scrolls sideways inside the pane)
 * and no more than the natural width (a narrower one keeps its size, it is not stretched). Whole pixels.
 */
export function diagramWidth(natural: number, available: number): number {
  if (!(natural > 0)) return Math.max(0, Math.floor(available));
  return Math.round(Math.min(natural, Math.max(natural * DIAGRAM_MIN_SHARE, available)));
}

/** The natural width of an SVG Mermaid made: the third number of its `viewBox`. null when it has none. */
export function naturalWidth(svg: string): number | null {
  const box = /<svg\b[^>]*?\sviewBox="([^"]*)"/i.exec(svg)?.[1];
  const parts = box?.trim().split(/[\s,]+/).map(Number);
  return parts && parts.length === 4 && parts.every(Number.isFinite) && parts[2] > 0 ? parts[2] : null;
}
