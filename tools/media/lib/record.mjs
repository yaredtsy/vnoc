// Frames for a GIF, and the marks that say what was happening when (docs/18-release-0.2/01-media.md §7.1, §7.2).
// The recording is never timed exactly and does not try to be: it keeps every frame, and what happens in the film as a list of marks. `edit.mjs` cuts the
// waits and speeds up the slow parts afterwards, and can do it again from the saved frames and marks without starting VS Code.
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
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
  }

  /**
   * The frame loop. A frame is `Page.captureScreenshot` through DevTools at the screen's own resolution (2560 x 1600 on a Retina Mac), as a JPEG because that is
   * fast; ffmpeg scales it to 1280 wide. Neither `page.screenshot({ scale: "css" })` nor a DevTools `clip.scale` may be used here: both change the page's scale
   * for the capture (the window flickers, hovers and clicks under way are cancelled, and a clip scale can leave the layout zoomed).
   */
  start() {
    this.#running = true;
    this.#t0 = performance.now();
    let ready;
    const warm = new Promise((resolve) => (ready = resolve));
    this.#loop = (async () => {
      const session = await this.#page.context().newCDPSession(this.#page);
      // the first captures are slow (a second or more); they are taken, thrown away, and the film's clock starts after them
      let warming = 6;
      for (let i = 0; this.#running; ) {
        if (this.#paused) {
          await new Promise((resolve) => setTimeout(resolve, 15));
          continue;
        }
        const t = this.elapsed;
        i++;
        const file = join(this.dir, `f${String(i).padStart(5, "0")}.jpg`);
        try {
          const { data } = await session.send("Page.captureScreenshot", { format: "jpeg", quality: 92, fromSurface: true });
          writeFileSync(file, Buffer.from(data, "base64"));
        } catch (error) {
          // the window went away (the scene failed): that error is the one to show, not this one
          if (this.#running) this.#error = error;
          ready();
          return;
        }
        if (warming > 0) {
          if (--warming === 0) {
            i = 0;
            this.frames = [];
            this.#t0 = performance.now();
            this.#cut = 0;
            ready();
          }
          continue;
        }
        this.frames.push({ file, t });
      }
    })();
    return warm;
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
    const seconds = this.elapsed;
    return { frames: this.frames.length, seconds, fps: this.frames.length / seconds };
  }

  /** Writes what the edit needs to run again without recording: `out/<scene>.frames.json` and `out/<scene>.marks.json`. */
  save() {
    writeFileSync(join(paths.out, `${this.name}.frames.json`), JSON.stringify({ frames: this.frames }, null, 1));
    writeFileSync(join(paths.out, `${this.name}.marks.json`), JSON.stringify(this.marks, null, 1));
  }
}
