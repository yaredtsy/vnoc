// The edit (docs/18-release-0.2/04-gifs-v2.md §6.3), from the saved frames, marks and beats of a recording: no VS Code, so a GIF can be edited again in seconds
// (`node run.mjs --encode-only g1`). First the machine (the automatic pass), then a person's edit list (`edits/<scene>.json`); the rules are in `timing.mjs`, which is
// pure and tested. This file reads and writes: the film, the list, ffmpeg.
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { paths, window as size } from "../config.mjs";
import { RULES, retime } from "./timing.mjs";

const FFMPEG = process.env.FFMPEG ?? "/opt/homebrew/bin/ffmpeg";
const FONT = ["/System/Library/Fonts/Menlo.ttc", "/System/Library/Fonts/Helvetica.ttc", "/Library/Fonts/Arial.ttf"].find((f) => existsSync(f));

/** The film as it was recorded: its frames (`{ file, t }`), where it ends, its marks and its beats. */
export function loadFilm(name) {
  const read = (suffix) => {
    const file = join(paths.out, `${name}.${suffix}.json`);
    if (!existsSync(file)) throw new Error(`${file} does not exist: record ${name} first (node run.mjs --only ${name})`);
    return JSON.parse(readFileSync(file, "utf8"));
  };
  const { frames, end } = read("frames");
  const missing = frames.find((f) => !existsSync(f.file));
  if (missing) throw new Error(`the frames of ${name} are gone (${missing.file}): record again`);
  return { name, frames, end: end ?? frames.at(-1).t + 0.5, marks: read("marks"), beats: read("beats") };
}

/** The edit list of a scene, written by a person: `edits/<scene>.json`. No file is no edits. Returns the entries and the text, for the review page. */
export function readEdits(name, dir = paths.edits) {
  const file = join(dir, `${name}.json`);
  if (!existsSync(file)) return { file, entries: [], text: null };
  const text = readFileSync(file, "utf8");
  try {
    return { file, entries: JSON.parse(text), text };
  } catch (error) {
    throw new Error(`${file} is not JSON: ${error.message}`);
  }
}

/** The list file ffmpeg reads: each frame with the time it is on screen (a duration of 0 leaves it out). */
function writeList(film, durations, file) {
  const lines = [];
  let last;
  for (const [i, f] of film.frames.entries()) {
    if (durations[i] <= 0.0005) continue;
    lines.push(`file '${f.file}'`, `duration ${durations[i].toFixed(4)}`);
    last = f;
  }
  lines.push(`file '${last.file}'`); // ffmpeg ignores the duration of the last entry unless it is repeated
  mkdirSync(join(file, ".."), { recursive: true });
  writeFileSync(file, lines.join("\n") + "\n");
}

/** Stretches of film (start, end) where the frames do not change for at least `freezeMin` s: ffmpeg's `freezedetect` on the recording as it was. `end` is undefined for one that runs to the end. */
function freezesOf(film) {
  const list = join(paths.out, "frames", film.name, "frames-raw.txt");
  writeList(film, film.frames.map((f, i) => (film.frames[i + 1]?.t ?? film.end) - f.t), list);
  const run = spawnSync(FFMPEG, ["-hide_banner", "-nostats", "-f", "concat", "-safe", "0", "-i", list, "-vf", `freezedetect=n=-60dB:d=${RULES.freezeMin}`, "-an", "-f", "null", "-"], {
    encoding: "utf8",
    maxBuffer: 64 << 20,
  });
  const starts = [...run.stderr.matchAll(/freeze_start: ([\d.]+)/g)].map((m) => Number(m[1]));
  const ends = [...run.stderr.matchAll(/freeze_end: ([\d.]+)/g)].map((m) => Number(m[1]));
  return starts.map((start, i) => ({ start, end: ends[i] }));
}

/** Edits a film: the automatic pass, then the edit list (`entries`). Returns what `timing.retime` does: the durations, the review film's beats, the lines to print. */
export function editFilm(film, { label, entries = [], file }) {
  const times = film.frames.map((f) => f.t);
  return retime({ times, end: film.end, marks: film.marks, beats: film.beats, freezes: freezesOf(film) }, entries, { label, file });
}

const SCALE = `scale=${size.width}:${size.height}:flags=lanczos`;

/** The GIF (two-pass palette, 20 fps, 256 colours) and the MP4 for posts, from the edited durations. Returns the sizes. */
export function encodeFilm(film, durations, { gif, mp4, fps = 20, colors = 256 }) {
  const list = join(paths.out, "frames", film.name, "frames-edit.txt");
  writeList(film, durations, list);
  const filter = `${SCALE},fps=${fps},split[a][b];[a]palettegen=max_colors=${colors}:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle`;
  execFileSync(FFMPEG, ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", list, "-vf", filter, "-loop", "0", gif], { stdio: "inherit" });
  if (mp4) execFileSync(FFMPEG, ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", list, "-vf", `${SCALE},fps=30`, "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20", mp4], { stdio: "inherit" });
  return { gifBytes: statSync(gif).size, mp4Bytes: mp4 ? statSync(mp4).size : 0 };
}

/**
 * The review film (04 §6.3): the film after the automatic pass, with the name of the beat and the seconds since it began burned in (`explore +0.8`). It is the film a
 * person watches to write the edit list, whose times are on this clock.
 */
export function encodeReview(film, auto, beats, file) {
  const list = join(paths.out, "frames", film.name, "frames-review.txt");
  writeList(film, auto, list);
  const clocks = beats.map((b) => {
    const name = b.name.replace(/[\\':%]/g, "");
    const since = `t-${b.start.toFixed(3)}`;
    // `+%{eif:…:d}.%{eif:…:d}`: whole seconds, then the tenth; the colons and commas inside the text are escaped for the filter graph
    return (
      `drawtext=fontfile='${FONT}':fontsize=22:fontcolor=white:box=1:boxcolor=black@0.65:boxborderw=7:x=12:y=10:enable='between(t,${b.start.toFixed(3)},${b.end.toFixed(3)})'` +
      `:text='${name} +%{eif\\:trunc(${since})\\:d}.%{eif\\:trunc(mod((${since})*10\\,10))\\:d}'`
    );
  });
  const filter = [SCALE, "fps=30", ...clocks, "format=yuv420p"].join(",");
  execFileSync(FFMPEG, ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", list, "-vf", filter, "-c:v", "libx264", "-crf", "20", file], { stdio: "inherit" });
}
