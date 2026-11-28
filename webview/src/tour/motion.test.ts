import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

// Reduce motion (docs/26-tours/design/05-states.md §6, §8): in the tour's styles every thing that moves (a fade, a pulse, the shimmer of grey bars) is switched off by the
// `prefers-reduced-motion` media query; the camera's pans are `motionMs` (lib/motion.ts). A new animation without its rule fails here.
const css = (name: string) => readFileSync(new URL(name, import.meta.url), "utf8");

test("every animation of the tour's styles has its reduced-motion rule", () => {
  for (const file of ["./stage.css", "./tour-map.css"]) {
    const text = css(file).replace(/\/\*[\s\S]*?\*\//g, "");
    // every `selector { declarations }` with no braces inside it; the ones of the media query are the ones that say `animation: none`
    const rules = Array.from(text.matchAll(/([^{}]+)\{([^{}]*)\}/g), (m) => ({ selector: m[1].replace(/@media[^{]*$/, "").replace(/@keyframes.*$/, "").trim(), body: m[2] }));
    const animated = rules.filter((r) => /\banimation:\s*(?!none)/.test(r.body) && r.selector !== "").map((r) => r.selector);
    const reduced = rules.filter((r) => /\banimation:\s*none/.test(r.body)).map((r) => r.selector);
    assert.ok(animated.length > 0, `${file} has animations to check`);
    assert.deepEqual(animated.filter((sel) => !reduced.includes(sel)), [], `${file}: an animation with no reduced-motion rule`);
  }
});

test("the camera's pans in a tour take their duration from the reduced-motion rule", () => {
  const canvas = readFileSync(new URL("./TourCanvas.tsx", import.meta.url), "utf8");
  assert.match(canvas, /const duration = \(\) => motionMs\(200, prefersReducedMotion\(\)\)/);
  assert.equal(Array.from(canvas.matchAll(/duration: (?!0\b)(?!duration\(\))/g)).length, 0, "no fixed duration");
});
