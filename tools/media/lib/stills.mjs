// `--stills`: the scene runs as it would for a film, but nothing is recorded. Where a film marks the start of a beat, this takes a picture of the beat that has just
// ended (the state it left, with its caption, its key badge and the arrow: what the viewer would see at the end of it), and `stop()` writes
// `out/<scene>/storyboard.html`: every beat in order, with its name, its caption and the keys pressed. Gate 1 of docs/18-release-0.2/04-gifs-v2.md §7: does it
// read as a story with no motion?
// It has the same surface as `Recording` (`start`, `beat`, `hold`, `during`, `begin`, `pause`, `resume`, `stop`, `save`), so a scene does not know the difference.
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { paths, window as size } from "../config.mjs";
import { Recording } from "./record.mjs";

const escape = (text) => text.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

export class Stills {
  #stage;
  #pending = null;
  /** `{ name, caption, keys, file, notes }`, in order */
  shots = [];

  constructor(stage, name) {
    this.#stage = stage;
    this.name = name;
    this.dir = join(paths.out, name);
    rmSync(this.dir, { recursive: true, force: true });
    mkdirSync(this.dir, { recursive: true });
    // the badge of a key stays until the next beat: the picture is taken long after a film's 0.8 s
    stage.stills = true;
  }

  async start() {}

  get elapsed() {
    return 0;
  }

  /** What a scene says about the beat that only a picture can check (a zoom, a count): `rec.note("zoom 1.00")`, shown under the picture. */
  note(text) {
    this.#pending?.notes.push(text);
  }

  /** An extra picture inside the beat, now (the palette is gone by the end of its beat): shown above the beat's own picture in the storyboard. A film has no use for it. */
  async look(label) {
    if (!this.#pending) throw new Error("look() is for inside a beat");
    await this.#stage.page.waitForTimeout(250);
    const file = `${String(this.shots.length + 1).padStart(2, "0")}-${this.#pending.name}-${label.replace(/\W+/g, "-")}.jpg`;
    await this.#stage.page.screenshot({ path: join(this.dir, file), type: "jpeg", quality: 90, scale: "device" });
    (this.#pending.looks ??= []).push({ file, label });
  }

  async beat(name) {
    await this.#flush();
    if (this.shots.some((s) => s.name === name) || this.#pending?.name === name) throw new Error(`the beat "${name}" is named twice`);
    this.#pending = { name, notes: [] };
    this.#stage.pressed.length = 0;
    await this.#stage.cursor.clearKey();
    console.log(`  beat ${this.shots.length + 1}: ${name}`);
  }

  async #flush() {
    const beat = this.#pending;
    if (!beat) return;
    this.#pending = null;
    const { page, cursor } = this.#stage;
    await page.waitForTimeout(350); // a glide or a fit of the view has ended
    const file = `${String(this.shots.length + 1).padStart(2, "0")}-${beat.name}.jpg`;
    // the page at the screen's own resolution, with everything the film draws over it
    await page.screenshot({ path: join(this.dir, file), type: "jpeg", quality: 90, scale: "device" });
    this.shots.push({ ...beat, file, caption: cursor.captionText, keys: [...this.#stage.pressed] });
  }

  /** A film stands still here; a still does not. */
  async hold() {}

  async during(_type, _label, fn) {
    return fn();
  }

  begin() {
    return () => {};
  }

  pause() {}
  resume() {}

  async stop() {
    await this.#flush();
    const file = join(this.dir, "storyboard.html");
    writeFileSync(file, this.#page());
    this.file = file;
    return { frames: this.shots.length, seconds: 0, fps: 0 };
  }

  save() {}

  #page() {
    const figures = this.shots
      .map(
        (s, i) => `    <figure>
      <figcaption><span class="n">${i + 1}</span> <b>${escape(s.name)}</b>${s.keys.length ? ` <span class="keys">keys: ${s.keys.map(escape).join(" · ")}</span>` : ""}</figcaption>
${(s.looks ?? []).map((l) => `      <p class="note">in the beat: ${escape(l.label)}</p>\n      <a href="${l.file}"><img src="${l.file}" width="${size.width}" height="${size.height}" alt="${escape(l.label)}"></a>`).join("\n")}
      <p class="note">${s.looks?.length ? "the end of the beat:" : ""}</p>
      <a href="${s.file}"><img src="${s.file}" width="${size.width}" height="${size.height}" alt="${escape(s.name)}"></a>
      <p class="caption">${s.caption ? `caption: “${escape(s.caption)}”` : "no caption"}</p>${s.notes.map((n) => `\n      <p class="note">${escape(n)}</p>`).join("")}
    </figure>`,
      )
      .join("\n");
    return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>${escape(this.name)}: storyboard</title>
<style>
  body { font: 15px/1.4 -apple-system, system-ui, sans-serif; background: #f4f4f5; color: #18181b; margin: 0; padding: 24px; }
  h1 { font-size: 20px; margin: 0 0 4px; }
  .lead { color: #52525b; margin: 0 0 24px; max-width: ${size.width}px; }
  figure { margin: 0 0 28px; }
  figcaption { margin-bottom: 6px; }
  .n { display: inline-block; min-width: 1.6em; padding: 0 6px; border-radius: 4px; background: #18181b; color: #fff; text-align: center; }
  .keys { color: #2563eb; margin-left: 8px; }
  img { display: block; border: 1px solid #a1a1aa; box-shadow: 0 2px 10px #0003; max-width: none; }
  .caption { margin: 6px 0 0; font-weight: 600; }
  .note { margin: 2px 0 0; color: #b45309; }
</style></head><body>
  <h1>${escape(this.name)}: storyboard (${this.shots.length} beats)</h1>
  <p class="lead">One picture per beat, taken at the end of it: what the viewer sees when the next beat begins, at the size of the GIF (${size.width} × ${size.height}). Click a picture for the full-resolution file.</p>
${figures}
</body></html>
`;
  }
}

/** A `Recording` for a film, `Stills` for `--stills`. */
export function recordingFor(stage, name, { stills = false } = {}) {
  return stills ? new Stills(stage, name) : new Recording(stage.page, name);
}
