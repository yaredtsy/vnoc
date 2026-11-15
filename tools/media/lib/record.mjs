// Frames for a GIF: screenshots taken in a loop, each with its time, so the GIF keeps the real timing even if the loop is uneven (01-media.md §7).
import { execFileSync } from "node:child_process";
import { mkdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { paths, window as size } from "../config.mjs";

const FFMPEG = process.env.FFMPEG ?? "/opt/homebrew/bin/ffmpeg";

export class Recording {
  #page;
  #dir;
  #frames = [];
  #running = false;
  #loop;
  #t0 = 0;
  #error;
  #paused = false;
  #pausedAt = 0;
  /** time spent paused: the film does not have it */
  #cut = 0;

  constructor(page, name) {
    this.#page = page;
    this.#dir = join(paths.work, "frames", name);
    rmSync(this.#dir, { recursive: true, force: true });
    mkdirSync(this.#dir, { recursive: true });
    this.name = name;
  }

  /**
   * The frame loop. A frame is `Page.captureScreenshot` through DevTools at the screen's own resolution (2560 x 1600 on a Retina Mac), as a JPEG because that is
   * fast; ffmpeg scales it to 1280 wide. Neither `page.screenshot({ scale: "css" })` nor a DevTools `clip.scale` may be used here: both change the page's scale
   * for the capture (the window flickers, hovers and clicks under way are cancelled, and a clip scale can leave the layout zoomed).
   */
  start() {
    this.#running = true;
    this.#t0 = performance.now();
    this.#loop = (async () => {
      const session = await this.#page.context().newCDPSession(this.#page);
      for (let i = 0; this.#running; ) {
        if (this.#paused) {
          await new Promise((resolve) => setTimeout(resolve, 15));
          continue;
        }
        const t = (performance.now() - this.#t0 - this.#cut) / 1000;
        i++;
        const file = join(this.#dir, `f${String(i).padStart(5, "0")}.jpg`);
        try {
          const { data } = await session.send("Page.captureScreenshot", { format: "jpeg", quality: 92, fromSurface: true });
          writeFileSync(file, Buffer.from(data, "base64"));
        } catch (error) {
          // the window went away (the scene failed): that error is the one to show, not this one
          if (this.#running) this.#error = error;
          return;
        }
        this.#frames.push({ file, t });
      }
    })();
  }

  /**
   * Stops taking frames (a load the viewer would only watch is a cut, not dead time): the frame before and the frame after are next to each other in the film.
   * Always paired with `resume()`.
   */
  pause() {
    this.#paused = true;
    this.#pausedAt = performance.now();
  }

  resume() {
    this.#cut += performance.now() - this.#pausedAt;
    this.#paused = false;
  }

  /** Seconds of film so far. */
  get elapsed() {
    return (performance.now() - this.#t0 - this.#cut) / 1000;
  }

  /** Waits (the recording goes on): the scene's `hold`. */
  hold(ms) {
    return this.#page.waitForTimeout(ms);
  }

  async stop() {
    this.#running = false;
    await this.#loop;
    if (this.#error) throw this.#error;
    const seconds = (performance.now() - this.#t0 - this.#cut) / 1000;
    return { frames: this.#frames.length, seconds, fps: this.#frames.length / seconds };
  }

  /** The list file ffmpeg reads: every frame with the time it was on screen; the last one for 1 s (the loop point). */
  #list() {
    const lines = this.#frames.map((f, i) => `file '${f.file}'\nduration ${(((this.#frames[i + 1]?.t ?? f.t + 1) - f.t)).toFixed(4)}`);
    lines.push(`file '${this.#frames.at(-1).file}'`); // ffmpeg ignores the duration of the last entry unless it is repeated
    const list = join(this.#dir, "frames.txt");
    writeFileSync(list, lines.join("\n") + "\n");
    return list;
  }

  /** The GIF (two-pass palette) and the MP4 for posts. Returns the sizes. */
  encode({ gif, mp4, fps = 12, colors = 128 }) {
    const list = this.#list();
    mkdirSync(paths.out, { recursive: true });
    const filter = `scale=${size.width}:${size.height}:flags=lanczos,fps=${fps},split[a][b];[a]palettegen=max_colors=${colors}:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle`;
    execFileSync(FFMPEG, ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", list, "-vf", filter, "-loop", "0", gif], { stdio: "inherit" });
    if (mp4) execFileSync(FFMPEG, ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", list, "-vf", `scale=${size.width}:${size.height}:flags=lanczos,fps=30`, "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20", mp4], { stdio: "inherit" });
    return { gifBytes: statSync(gif).size, mp4Bytes: mp4 ? statSync(mp4).size : 0 };
  }
}
