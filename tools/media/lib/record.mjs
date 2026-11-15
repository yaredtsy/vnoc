// Frames for a GIF, and the marks that say what was happening when (docs/18-release-0.2/01-media.md §7.1, docs/18-release-0.2/04-gifs-v2.md §6).
// The recording is never timed exactly and does not try to be: it keeps every frame, and what happens in the film as a list of marks. `edit.mjs` cuts the
// waits and speeds up the slow parts afterwards, and can do it again from the saved frames and marks without starting VS Code.
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { writeFile } from "node:fs/promises";
import { join } from "node:path";

import { paths } from "../config.mjs";

/**
 * What a mark says, and what the edit does with it (§7.2):
 *   wait    from an action to the page being settled (a fetch, Explore loading)       cut to 0.3 s
 *   drag    a drag of the canvas                                                      2× faster
 *   glide   the arrow moving                                                          1× (1.5× if longer than 0.5 s)
 *   typing  keys typed (G2)                                                           1.5× if longer than 2 s
 *   reveal  what the GIF is for: cards drawn, Explore's switch, the doc panel          never changed
 *   hold    the film stands still on purpose (`kind`: first, after, read, end)        kept; shortened only by the length budget
 * The unmarked time between marks is what the script did not know about: a stretch of unchanged frames in it is cut to 0.7 s and reported.
 */
export const MARK_TYPES = ["wait", "drag", "glide", "typing", "reveal", "hold"];

export class Recording {
  #page;
  #running = false;
  #loop;
  #t0 = 0;
  #error;
  #paused = false;
  #pausedAt = 0;
  /** time spent paused: the film does not have it */
  #cut = 0;
  /** how many marks are open: a mark inside another one belongs to the outer one */
  #open = 0;

  constructor(page, name) {
    this.#page = page;
    this.name = name;
    this.dir = join(paths.out, "frames", name);
    rmSync(this.dir, { recursive: true, force: true });
    mkdirSync(this.dir, { recursive: true });
    /** `{ file, t }`: a frame and the film's second it was taken at */
    this.frames = [];
    /** `{ type, label, kind?, from, to }`, in the film's seconds */
    this.marks = [];
    /** `{ name, t }`: where each beat of the script begins, in the film's seconds (04 §6.1: the edit list refers to beats by name) */
    this.beats = [];
  }

  /**
   * Starts taking frames; the promise it returns is settled when the first one is in (04 §6.2). DevTools `Page.startScreencast`: a frame arrives when the screen
   * changes, with its own time, at the screen's own resolution (1920 x 1200 on a Retina Mac), as a JPEG. A stretch of film with no change has no frames: the last
   * frame stays until the next. It changes neither the page's scale nor a hover (measured). It replaced a `Page.captureScreenshot` loop, which gave 13.7 different
   * pictures a second while the screen moved, against 45 for the screencast on the same scene. Neither `page.screenshot({ scale: "css" })` nor a DevTools
   * `clip.scale` may be used while a film runs: both change the page's scale for the capture (the window flickers, hovers and clicks under way are cancelled).
   */
  start() {
    this.#running = true;
    this.#t0 = performance.now();
    let ready;
    const warm = new Promise((resolve) => (ready = resolve));
    this.#loop = this.#screencast(ready);
    return warm;
  }

  async #screencast(ready) {
    const session = await this.#page.context().newCDPSession(this.#page);
    const writes = [];
    let first;
    let offset = 0;
    let last = -Infinity;
    session.on("Page.screencastFrame", (frame) => {
      // every frame is acknowledged, or the stream stops
      session.send("Page.screencastFrameAck", { sessionId: frame.sessionId }).catch(() => {});
      if (!this.#running || this.#paused) return;
      const stamp = frame.metadata.timestamp; // seconds, the time the screen was captured
      const raw = (performance.now() - this.#t0) / 1000;
      if (first === undefined) {
        // the first frame is the screen as it is: the film starts here
        first = stamp;
        this.#t0 = performance.now();
        this.#cut = 0;
        offset = 0;
        ready();
      } else {
        // a frame's own time, put on the clock the marks use (`elapsed`), with the delay of the transfer taken out: the smallest delay seen is the real one
        offset = Math.min(offset, raw - (stamp - first));
      }
      // times can arrive a few milliseconds out of order: the film's are never
      const t = Math.max(last + 0.001, stamp - first + offset - this.#cut / 1000, 0);
      last = t;
      const file = join(this.dir, `f${String(this.frames.length + 1).padStart(5, "0")}.jpg`);
      this.frames.push({ file, t });
      writes.push(writeFile(file, Buffer.from(frame.data, "base64")).catch((error) => (this.#error ??= error)));
    });
    await session.send("Page.startScreencast", { format: "jpeg", quality: 92, maxWidth: 4096, maxHeight: 4096, everyNthFrame: 1 });
    // runs until `stop()`
    while (this.#running) await new Promise((resolve) => setTimeout(resolve, 20));
    await session.send("Page.stopScreencast").catch(() => {});
    await Promise.all(writes);
    ready();
  }

  /** Seconds of film so far. */
  get elapsed() {
    return (performance.now() - this.#t0 - this.#cut) / 1000;
  }

  /**
   * Opens a mark; the function it returns closes it. A mark inside another is ignored (the outer one is what the edit sees): a drag inside a wait is part of
   * the wait.
   */
  begin(type, label = "", extra = {}) {
    if (this.#open > 0) return () => {};
    this.#open++;
    const mark = { type, label, ...extra, from: this.elapsed };
    return () => {
      mark.to = this.elapsed;
      this.marks.push(mark);
      this.#open--;
    };
  }

  /** Runs `fn` as one mark. */
  async during(type, label, fn, extra) {
    const end = this.begin(type, label, extra);
    try {
      return await fn();
    } finally {
      end();
    }
  }

  /**
   * The start of a beat of the script: a named mark (`rec.beat("explore")`). The edit list of 04 §6.3 refers to beats by name, and its times are relative to
   * them, so a new recording does not make the list wrong. A name is used once. Async because the stills (`stills.mjs`) take a picture here.
   */
  async beat(name) {
    if (this.beats.some((b) => b.name === name)) throw new Error(`the beat "${name}" is named twice`);
    this.beats.push({ name, t: this.elapsed });
    if (process.env.MEDIA_TIMES) console.log(`  ${this.elapsed.toFixed(1)} s  ${name}`);
  }

  /** An extra picture inside a beat for the storyboard of `--stills`; a film has no use for it. */
  async look() {}

  /** A remark for the storyboard of `--stills` (a zoom, a count); a film has no use for it. */
  note() {}

  /** The film stands still for `ms`, on purpose. `kind`: `first` (the first frame), `after` (after an action), `read` (something new to read), `end` (the last frame). */
  hold(ms, kind = "after") {
    return this.during("hold", "", () => this.#page.waitForTimeout(ms), { kind });
  }

  /**
   * Stops taking frames: a true cut, for what the viewer must not see at all (the product's zoom jump when Explore opens). Always paired with `resume()`, and
   * never with a mark open inside it. A wait that is only slow is a `wait` mark, not a pause.
   */
  pause() {
    this.#paused = true;
    this.#pausedAt = performance.now();
  }

  resume() {
    this.#cut += performance.now() - this.#pausedAt;
    this.#paused = false;
  }

  async stop() {
    this.#running = false;
    await this.#loop;
    if (this.#error) throw this.#error;
    this.end = this.elapsed;
    const seconds = this.end;
    return { frames: this.frames.length, seconds, fps: this.frames.length / seconds, ...intervalStats(this.frames, this.end) };
  }

  /** Writes what the edit needs to run again without recording: `out/<scene>.frames.json`, `.marks.json` and `.beats.json`. */
  save() {
    writeFileSync(join(paths.out, `${this.name}.beats.json`), JSON.stringify(this.beats, null, 1));
    // `end`: where the last frame stops (a frame stays until the next one, and the last until the film ends)
    writeFileSync(join(paths.out, `${this.name}.frames.json`), JSON.stringify({ end: this.end, frames: this.frames }, null, 1));
    writeFileSync(join(paths.out, `${this.name}.marks.json`), JSON.stringify(this.marks, null, 1));
  }
}

/** How the frames are spread in time: the interval between one and the next (milliseconds) and the longest stretches with no new frame. */
export function intervalStats(frames, end) {
  const gaps = frames.map((f, i) => ((frames[i + 1]?.t ?? end) - f.t) * 1000).sort((a, b) => a - b);
  const at = (q) => gaps[Math.min(gaps.length - 1, Math.floor(q * gaps.length))] ?? 0;
  return { medianMs: at(0.5), p95Ms: at(0.95), maxGapMs: gaps.at(-1) ?? 0, over250: gaps.filter((g) => g > 250).length };
}
