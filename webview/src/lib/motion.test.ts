import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { motionMs, spinnerIcon } from "./motion.ts";

// the rule of 09 §6: with reduced motion, nothing pulses, and a spinner is a static dot
test("a spinner turns; with reduced motion it is a dot that stands still", () => {
  assert.deepEqual(spinnerIcon(false), { name: "loading", spin: true });
  assert.deepEqual(spinnerIcon(true), { name: "circle-filled", spin: false });
});

test("a script-driven move (fit view) lasts as long as asked, and not at all with reduced motion", () => {
  assert.equal(motionMs(300, false), 300);
  assert.equal(motionMs(300, true), 0);
});

const src = fileURLToPath(new URL("..", import.meta.url));
const files = (dir: string): string[] => readdirSync(dir).flatMap((f) => (statSync(join(dir, f)).isDirectory() ? files(join(dir, f)) : [join(dir, f)]));
// with `/` on every OS, so the checks below can name folders the same way on Windows
const all = files(src).map((f) => f.replaceAll("\\", "/")).filter((f) => !f.endsWith(".test.ts"));
const text = (file: string) => readFileSync(file, "utf8");
const rel = (file: string) => file.slice(src.replaceAll("\\", "/").length);

test("no `animate-` class is used without `motion-safe:`, but the UI kit's (its tooltips and menus fade in; the page's reduce-motion rule makes that instant)", () => {
  for (const file of all.filter((f) => /\.tsx?$/.test(f) && !f.includes("/components/ui/"))) {
    for (const match of text(file).matchAll(/(\S*)animate-\S+/g)) assert.ok(match[1].endsWith("motion-safe:") || match[0].startsWith("tw-animate"), `${rel(file)}: ${match[0]}`);
  }
});

test("only Spinner draws a turning glyph", () => {
  for (const file of all.filter((f) => /\.tsx?$/.test(f))) {
    if (file.endsWith("components/Spinner.tsx") || file.endsWith("lib/motion.ts")) continue;
    assert.ok(!text(file).includes("codicon-modifier-spin") || file.endsWith(".css"), `${rel(file)} spins a glyph that the dot rule does not reach`);
  }
});

test("the page says what reduced motion does: instant transitions and animations, no pulse on a revealed card, no spin", () => {
  const style = text(join(src, "style.css"));
  assert.match(style, /@media \(prefers-reduced-motion: reduce\)\s*\{[^}]*animation-duration: 0\.01ms !important/);
  assert.match(style, /transition-duration: 0\.01ms !important/);
  const card = text(join(src, "node-card/node-card.css"));
  assert.match(card, /@media \(prefers-reduced-motion: reduce\) \{ \.node-card\.revealed \{ animation: none; \} \}/);
  assert.match(card, /\.codicon-modifier-spin \{ animation: none; \}/);
});

test("every tailwind transition outside the kit is `motion-safe:` or has no duration", () => {
  for (const file of all.filter((f) => /\.tsx$/.test(f) && !f.includes("/components/ui/"))) {
    for (const match of text(file).matchAll(/(\S*)\btransition-(transform|opacity|colors)\b/g)) {
      const line = text(file).split("\n").find((l) => l.includes(match[0])) ?? "";
      assert.ok(match[1].endsWith("motion-safe:") || /duration-0\b/.test(line), `${rel(file)}: ${match[0]}`);
    }
  }
});
