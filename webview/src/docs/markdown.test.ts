import assert from "node:assert/strict";
import { test } from "node:test";

import { renderMarkdown, withoutFrontmatter } from "./markdown.ts";

test("the frontmatter is not shown, the heading is", () => {
  const html = renderMarkdown("---\nnode: X\nhint: a::b\n---\n# Why we parse twice\n\nText.");
  assert.match(html, /<h1>Why we parse twice<\/h1>/);
  assert.doesNotMatch(html, /node: X|hint/);
  assert.equal(withoutFrontmatter("no frontmatter"), "no frontmatter");
});

test("raw HTML from a doc is shown as text, never run", () => {
  const html = renderMarkdown('<script>alert(1)</script>\n\n<img src=x onerror="alert(1)">');
  assert.doesNotMatch(html, /<script|<img/);
  assert.match(html, /&lt;script&gt;/);
});

test("a link goes to the host, and a javascript: link is not a link at all", () => {
  assert.match(renderMarkdown("[docs](https://example.com/a?b=1&c=2)"), /<a href="#" data-href="https:\/\/example.com\/a\?b=1&amp;c=2">docs<\/a>/);
  assert.doesNotMatch(renderMarkdown("[x](javascript:alert(1))"), /<a /);
});

test("an image shows its alt text", () => {
  assert.equal(renderMarkdown("![the diagram](img/a.png)").trim(), '<p><span class="missing-image">the diagram</span></p>');
});
