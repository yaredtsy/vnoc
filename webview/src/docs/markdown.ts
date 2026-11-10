// Markdown → HTML for the doc panel. Docs arrive by `git pull` from other people, so they are untrusted: raw HTML is off (no `<script>`,
// no inline HTML at all), and markdown-it already refuses `javascript:` links. Pure, so it is tested without a browser.

import MarkdownIt from "markdown-it";

const md = new MarkdownIt({ html: false, linkify: true });

// Images with relative paths do not load in 0.1 (they need `asWebviewUri` and a wider `localResourceRoots`): show the alt text.
md.renderer.rules.image = (tokens, i) => `<span class="missing-image">${md.utils.escapeHtml(tokens[i].content || "image")}</span>`;

// A link is not followed inside the webview: the click goes to the host. The target travels in `data-href`.
md.renderer.rules.link_open = (tokens, i) => `<a href="#" data-href="${md.utils.escapeHtml(String(tokens[i].attrGet("href") ?? ""))}">`;

/** The doc without its `---` frontmatter (the title comes from the first heading, which stays). */
export function withoutFrontmatter(text: string): string {
  const match = /^---\r?\n[\s\S]*?\r?\n---\r?\n/.exec(text);
  return match ? text.slice(match[0].length) : text;
}

export function renderMarkdown(text: string): string {
  return md.render(withoutFrontmatter(text));
}
