// out/review.html (docs/18-release-0.2/04-gifs-v2.md §6.3, gate 2 and 3): what a person looks at to write the edit list. The GIF at 880 px, the width GitHub shows a README
// image at, on GitHub's dark and light backgrounds; the review film with its clock; the beats on that clock; the edit list as text; what the automatic pass did.
import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { paths } from "../config.mjs";

const escape = (text) => String(text).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

/** Keeps what the page needs of a scene (`out/<id>.review.json`), so a page can show every scene that was edited, not only the one this run did. */
export function saveSceneReview(info) {
  writeFileSync(join(paths.out, `${info.id}.review.json`), JSON.stringify(info, null, 1));
}

/** Writes `out/review.html` for the scenes that have a saved review. Returns its path. */
export function writeReviewPage(ids) {
  const scenes = ids.filter((id) => existsSync(join(paths.out, `${id}.review.json`))).map((id) => JSON.parse(readFileSync(join(paths.out, `${id}.review.json`), "utf8")));
  const version = (file) => (existsSync(join(paths.out, file)) ? statSync(join(paths.out, file)).mtimeMs.toFixed(0) : "0");
  const sections = scenes
    .map((s) => {
      const gif = `${s.gif}?v=${version(s.gif)}`;
      return `<section>
  <h2>${escape(s.id)}: ${escape(s.gif)}</h2>
  <p class="facts">recorded ${s.recorded.toFixed(1)} s · after the automatic pass ${s.autoSeconds.toFixed(1)} s · <b>the GIF ${s.seconds.toFixed(1)} s</b>, ${(s.gifBytes / 1e6).toFixed(2)} MB (the budget is 6 MB) · ${s.frames} frames</p>
  <h3>The GIF at 880 px, as GitHub shows it</h3>
  <div class="backgrounds">
    <div class="dark"><img src="${gif}" width="880" alt="${escape(s.id)} on a dark background"></div>
    <div class="light"><img src="${gif}" width="880" alt="${escape(s.id)} on a light background"></div>
  </div>
  <h3>The review film: after the automatic pass, with the clock (<code>beat +seconds since it began</code>)</h3>
  <video src="${s.review}?v=${version(s.review)}" controls width="${s.width}" preload="auto"></video>
  <div class="two">
    <div>
      <h3>The beats, on the review film's clock</h3>
      <table><tr><th>beat</th><th>begins</th><th>ends</th><th>long</th></tr>
${s.beats.map((b) => `        <tr><td>${escape(b.name)}</td><td>${b.start.toFixed(1)} s</td><td>${b.end.toFixed(1)} s</td><td>${(b.end - b.start).toFixed(1)} s</td></tr>`).join("\n")}
      </table>
      <h3>What the automatic pass did</h3>
      <pre>${escape(s.lines.join("\n"))}</pre>
    </div>
    <div>
      <h3>The edit list: <code>${escape(s.editsFile)}</code></h3>
      <pre>${s.editsText === null ? "(no file: no manual edits)" : escape(s.editsText)}</pre>
      <p class="hint">Write it while you watch the film: times are seconds since the beat began, on the clock burned into the film. <code>cut</code> removes a stretch, <code>speed</code> plays it faster
      (the whole beat with no <code>from</code>/<code>to</code>), <code>hold</code> sets how long the beat's last hold stays. Then <code>node run.mjs --encode-only ${escape(s.id)}</code>: a few seconds, no VS Code.
      Guides: up to 1.5× where the cursor moves, up to 3× for typing, nothing in the 0.3 s after a click.</p>
<pre class="format">[
  { "beat": "show-calls", "from": 0.4, "to": 1.6, "cut": true },
  { "beat": "explore",    "from": 0.0, "to": 0.9, "speed": 1.5 },
  { "beat": "end",        "hold": 3.0 }
]</pre>
    </div>
  </div>
</section>`;
    })
    .join("\n");
  const html = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>review</title>
<style>
  body { font: 15px/1.45 -apple-system, system-ui, sans-serif; background: #f4f4f5; color: #18181b; margin: 0; padding: 24px 32px 64px; }
  h1 { font-size: 22px; margin: 0 0 16px; }
  h2 { font-size: 19px; margin: 32px 0 4px; }
  h3 { font-size: 14px; margin: 20px 0 6px; color: #3f3f46; }
  .facts { margin: 0 0 4px; color: #3f3f46; }
  .backgrounds { display: flex; gap: 16px; flex-wrap: wrap; }
  .backgrounds > div { padding: 16px; border: 1px solid #8884; border-radius: 6px; }
  .dark { background: #0d1117; }
  .light { background: #ffffff; }
  img { display: block; }
  video { display: block; max-width: 100%; background: #000; }
  .two { display: flex; gap: 32px; flex-wrap: wrap; }
  .two > div { min-width: 360px; flex: 1; }
  table { border-collapse: collapse; }
  th, td { text-align: left; padding: 2px 14px 2px 0; font-variant-numeric: tabular-nums; }
  pre { background: #fff; border: 1px solid #d4d4d8; border-radius: 4px; padding: 10px 12px; overflow: auto; font: 13px/1.4 Menlo, monospace; }
  .format { color: #71717a; }
  .hint { color: #52525b; max-width: 640px; }
  code { font: 13px Menlo, monospace; }
</style></head><body>
  <h1>review: ${scenes.map((s) => escape(s.id)).join(", ") || "nothing edited yet"}</h1>
${sections}
</body></html>
`;
  const file = join(paths.out, "review.html");
  writeFileSync(file, html);
  return file;
}
