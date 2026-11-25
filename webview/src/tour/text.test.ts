import assert from "node:assert/strict";
import { test } from "node:test";

import { type MermaidLike, diagramWidth, drawDiagram, linkTarget, mermaidTheme, naturalWidth, renderText } from "./text.ts";

test("a paragraph keeps its words, `code` and emphasis and strong are marks", () => {
  assert.equal(renderText("It checks the stock.").html, "<p>It checks the stock.</p>\n");
  assert.equal(renderText("reads `quantity` units, *never* more, **always** less").html, "<p>reads <code>quantity</code> units, <em>never</em> more, <strong>always</strong> less</p>\n");
  assert.equal(renderText("").html, "");
});

test("a <b> and a <script> stay text: nothing the text says is HTML", () => {
  const { html } = renderText("a <b>bold</b> <script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\n<div>block</div>");
  assert.ok(!/<b>|<script|<img|<div>/.test(html), html);
  assert.ok(html.includes("&lt;b&gt;bold&lt;/b&gt;") && html.includes("&lt;script&gt;alert(1)&lt;/script&gt;"), html);
  assert.ok(renderText("`<i>x</i>`").html.includes("<code>&lt;i&gt;x&lt;/i&gt;</code>"));
});

test("a list, a table and a quote render, and a table scrolls in its own box", () => {
  assert.equal(renderText("- one\n- two").html, "<ul>\n<li>one</li>\n<li>two</li>\n</ul>\n");
  assert.equal(renderText("1. a\n2. b").html, "<ol>\n<li>a</li>\n<li>b</li>\n</ol>\n");
  const table = renderText("| a | b |\n|---|---|\n| 1 | 2 |").html;
  assert.ok(table.startsWith('<div class="tour-table"><table>') && table.includes("<th>a</th>") && table.includes("<td>2</td>") && table.trimEnd().endsWith("</table></div>"), table);
  assert.ok(renderText("> why\n> it is so").html.startsWith("<blockquote>"));
});

test("a code fence is plain code without a highlighter, and coloured with one", () => {
  const plain = renderText("```python\nx = 1 < 2\n```").html;
  assert.equal(plain, '<pre class="tour-fence"><code>x = 1 &lt; 2\n</code></pre>\n');
  const seen: string[] = [];
  const coloured = renderText("```py\nx = 1\n```", {
    languages: ["python"],
    fenceLanguage: (info, loaded) => (info === "py" && loaded.includes("python") ? "python" : null),
    highlight: (code, lang) => (seen.push(`${lang}:${code}`), `<pre class="shiki"><span>${code}</span></pre>`),
  }).html;
  assert.deepEqual(seen, ["python:x = 1\n"]);
  assert.equal(coloured, '<div class="tour-fence-box"><pre class="shiki"><span>x = 1\n</span></pre></div>\n');
  // a language the highlighter does not have is plain, not an error
  const unknown = renderText("```cobol\nMOVE 1\n```", { languages: ["python"], fenceLanguage: () => null, highlight: () => "<b>never</b>" }).html;
  assert.ok(unknown.includes('class="tour-fence"') && !unknown.includes("never"), unknown);
});

test("a mermaid fence is a placeholder with its source kept: nothing is loaded to render it", () => {
  let loads = 0;
  const { html, diagrams } = renderText("The flow:\n\n```mermaid\nflowchart LR\n  a --> b\n```\n\nand another:\n\n```mermaid\nflowchart TD\n  c --> d\n```");
  assert.deepEqual(diagrams, ["flowchart LR\n  a --> b\n", "flowchart TD\n  c --> d\n"]);
  assert.ok(html.includes('class="tour-diagram" data-diagram="0"') && html.includes('data-diagram="1"'), html);
  assert.ok(html.includes("a --&gt; b"), "the source is in the placeholder until it is drawn");
  assert.equal(loads, 0, "rendering a text never brings Mermaid in");
  void loads;
});

test("a diagram is drawn by the loaded Mermaid, strict, in the theme of VS Code's", async () => {
  const calls: string[] = [];
  const fake: MermaidLike = {
    initialize: (c) => calls.push(`init ${c.securityLevel} ${c.theme} ${c.startOnLoad}`),
    render: async (id, source) => (calls.push(`render ${id} ${source.trim()}`), { svg: "<svg></svg>" }),
  };
  let loaded = 0;
  const drawn = await drawDiagram(async () => (loaded++, fake), "flowchart LR\n a --> b", "d1", mermaidTheme("github-dark"));
  assert.deepEqual(drawn, { ok: true, svg: "<svg></svg>" });
  assert.deepEqual(calls, ["init strict dark false", "render d1 flowchart LR\n a --> b"]);
  assert.equal(loaded, 1);
  assert.equal(mermaidTheme("github-light"), "default");
});

test("a diagram that does not parse, or a Mermaid that does not load, is a note and never an exception", async () => {
  const broken: MermaidLike = { initialize: () => {}, render: async () => Promise.reject(new Error("Parse error on line 2")) };
  const a = await drawDiagram(async () => broken, "flowchart LR\n a -->", "d2", "default");
  assert.equal(a.ok, false);
  assert.ok(!a.ok && /could not be drawn/.test(a.note) && !/Parse error/.test(a.note), "one line: the source is shown, the parser's words are not");
  const b = await drawDiagram(() => Promise.reject(new Error("chunk failed")), "x", "d3", "dark");
  assert.equal(b.ok, false);
  const empty: MermaidLike = { initialize: () => {}, render: async () => ({ svg: "  " }) };
  assert.equal((await drawDiagram(async () => empty, "x", "d4", "dark")).ok, false);
});

test("a link never navigates: it is a click for the host, and only http(s) goes", () => {
  const { html } = renderText("see [the docs](https://example.com/a?b=1&c=2) or [x](javascript:alert(1)) or https://example.org/z");
  assert.ok(!/ href="https?:/.test(html), "no href holds an address: a click is the host's");
  assert.ok(!/<a [^>]*javascript:/.test(html) && html.includes("[x](javascript:alert(1))"), "a javascript: link is not a link: it stays as text");
  assert.ok(html.includes('<a href="#" data-href="https://example.com/a?b=1&amp;c=2">the docs</a>'), html);
  assert.ok(html.includes('data-href="https://example.org/z"'), "a bare address is a link");
  assert.equal(linkTarget("https://example.com"), "https://example.com");
  assert.equal(linkTarget("HTTP://example.com"), "HTTP://example.com");
  assert.equal(linkTarget("javascript:alert(1)"), null);
  assert.equal(linkTarget("../README.md"), null);
  assert.equal(linkTarget(null), null);
});

test("an image is its alt text, and never an <img>", () => {
  const { html } = renderText("![the shop](http://x/y.png)");
  assert.ok(!html.includes("<img") && html.includes('<span class="tour-image">the shop</span>'), html);
  assert.ok(!renderText("![](x)").html.includes("<img"));
});

test("a diagram is drawn at the pane's width, never under 80% of its natural size, never over it", () => {
  assert.equal(diagramWidth(1000, 400), 800, "a wide one is held at 80% and scrolls");
  assert.equal(diagramWidth(1000, 900), 900, "between 80% and 100% it fits the pane");
  assert.equal(diagramWidth(1000, 1400), 1000, "a pane wider than the diagram does not stretch it");
  assert.equal(diagramWidth(300, 400), 300, "a narrower one keeps its natural size");
  assert.equal(diagramWidth(300, 100), 240, "even a small one is held at 80%");
  assert.equal(diagramWidth(0, 400), 400, "no known size: the pane's width");
});

test("the natural width of Mermaid's SVG is its viewBox", () => {
  assert.equal(naturalWidth('<svg id="d" width="100%" style="max-width: 584.5px;" viewBox="0 0 584.5 300" role="graphics-document">'), 584.5);
  assert.equal(naturalWidth('<svg viewBox="-8, -8, 120 40">'), 120);
  assert.equal(naturalWidth("<svg></svg>"), null);
  assert.equal(naturalWidth('<svg viewBox="0 0 0 10">'), null);
});
