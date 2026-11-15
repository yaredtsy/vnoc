// The edit (docs/18-release-0.2/01-media.md §7.2): cut the waits, speed up the slow parts, after recording. It only changes how long each frame is on screen (a
// duration of 0 drops the frame); ffmpeg's `fps=12` does the rest. It needs the saved frames and marks, not VS Code, so a GIF can be edited again without
// recording again: `node run.mjs --encode-only g1`.
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { paths, window as size } from "../config.mjs";

const FFMPEG = process.env.FFMPEG ?? "/opt/homebrew/bin/ffmpeg";

/** What the edit does, in one place (§7.2). */
export const RULES = {
  waitKeep: 0.3, // a wait is cut to this
  dragSpeed: 2,
  glideLong: 0.5, // a glide longer than this is sped up
  glideSpeed: 1.5,
  typingLong: 2, // a typing longer than this is sped up
  typingSpeed: 1.5,
  freezeMin: 0.7, // unmarked frames that do not change for this long are cut to freezeKeep
  freezeKeep: 0.7,
  freezeNoise: "-60dB",
  floors: { first: 0.4, after: 0.4, read: 0.8, end: 2.0 }, // the budget shortens holds down to these
  maxSpeedUp: 1.15, // and then everything but the protected, at most this
  protectTail: 2.0, // the last seconds are never sped up or cut
};

export function loadFilm(name) {
  const read = (suffix) => {
    const file = join(paths.out, `${name}.${suffix}.json`);
    if (!existsSync(file)) throw new Error(`${file} does not exist: record ${name} first (node run.mjs --only ${name})`);
    return JSON.parse(readFileSync(file, "utf8"));
  };
  const { frames } = read("frames");
  const missing = frames.find((f) => !existsSync(f.file));
  if (missing) throw new Error(`the frames of ${name} are gone (${missing.file}): record again`);
  return { name, frames, marks: read("marks") };
}

/** The durations the film's frames have as recorded: until the next frame; the last for half a second. */
const durationsOf = (frames) => frames.map((f, i) => (frames[i + 1]?.t ?? f.t + 0.5) - f.t);

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
  writeFileSync(file, lines.join("\n") + "\n");
}

/** Stretches of film (start, end) where the frames do not change for at least `freezeMin` s. `end` is undefined for one that runs to the end. */
function freezesOf(film, durations) {
  const list = join(paths.out, "frames", film.name, "frames-raw.txt");
  writeList(film, durations, list);
  const run = spawnSync(FFMPEG, ["-hide_banner", "-nostats", "-f", "concat", "-safe", "0", "-i", list, "-vf", `freezedetect=n=${RULES.freezeNoise}:d=${RULES.freezeMin}`, "-an", "-f", "null", "-"], {
    encoding: "utf8",
    maxBuffer: 64 << 20,
  });
  const starts = [...run.stderr.matchAll(/freeze_start: ([\d.]+)/g)].map((m) => Number(m[1]));
  const ends = [...run.stderr.matchAll(/freeze_end: ([\d.]+)/g)].map((m) => Number(m[1]));
  return starts.map((start, i) => ({ start, end: ends[i] }));
}

export function readManual(name) {
  const file = join(paths.out, `${name}.edit.json`);
  if (!existsSync(file)) return [];
  const { manual } = JSON.parse(readFileSync(file, "utf8"));
  return Array.isArray(manual) ? manual : [];
}

const fmt = (n) => n.toFixed(1);

/**
 * Edits a film. Returns the duration of every frame, the lines to print, and the segments for `out/<scene>.edit.json`. `target`: the scene's length in seconds; none, no budget (04 §6).
 * `manual` entries (from the edit file, written by a person) are applied after the automatic edit: `{ from, to, speed }` or `{ cut: [from, to] }`, in recorded seconds.
 */
export function editFilm(film, { label, target = Infinity, manual = [] }) {
  const { frames, marks } = film;
  const d = durationsOf(frames);
  const mid = frames.map((f, i) => f.t + d[i] / 2);
  const recorded = d.reduce((a, b) => a + b, 0);
  const out = d.slice();
  const within = (a, b) => frames.flatMap((_, i) => (mid[i] >= a && mid[i] < b ? [i] : []));
  // A frame can be on screen for a long time (a capture that stalled), so a cut keeps a window of time, not whole frames: each frame keeps the part of its time
  // that lies inside [from, to].
  const keepWindow = (idx, from, to) => {
    for (const i of idx) out[i] = Math.min(out[i], Math.max(0, Math.min(frames[i].t + d[i], to) - Math.max(frames[i].t, from)));
  };
  const segments = marks.map((m) => ({ ...m, length: m.to - m.from, idx: within(m.from, m.to), note: "" }));
  const extra = []; // freezes and manual edits: segments the marks did not have

  // 1. the marks
  for (const s of segments) {
    if (s.type === "wait" && s.length > RULES.waitKeep) {
      // what is kept is the end of the wait: the loaded state. Only frames *taken* in that window: a frame taken earlier can still be the latest picture while the
      // capture stalls, and it would show the state from before the load (a spinner, "Activating Extensions…")
      const from = s.to - RULES.waitKeep;
      const taken = s.idx.filter((i) => frames[i].t >= from);
      if (taken.length > 0) {
        for (const i of s.idx) out[i] = taken.includes(i) ? Math.min(out[i], Math.max(0, s.to - frames[i].t)) : 0;
      } else {
        keepWindow(s.idx, from, s.to); // no frame was taken in the window: the last one is all there is
      }
      s.note = `cut to ${RULES.waitKeep} s`;
    } else if (s.type === "drag") {
      for (const i of s.idx) out[i] /= RULES.dragSpeed;
      s.note = `${RULES.dragSpeed}×`;
    } else if (s.type === "glide" && s.length > RULES.glideLong) {
      for (const i of s.idx) out[i] /= RULES.glideSpeed;
      s.note = `${RULES.glideSpeed}×`;
    } else if (s.type === "typing" && s.length > RULES.typingLong) {
      for (const i of s.idx) out[i] /= RULES.typingSpeed;
      s.note = `${RULES.typingSpeed}×`;
    }
  }

  // 2. what the marks miss: a stretch of unchanged frames in the unmarked time
  const sorted = [...marks].sort((a, b) => a.from - b.from);
  const gaps = [];
  let at = 0;
  for (const m of sorted) {
    if (m.from > at) gaps.push([at, m.from]);
    at = Math.max(at, m.to);
  }
  if (at < recorded) gaps.push([at, recorded]);
  for (const { start, end = recorded } of freezesOf(film, d)) {
    for (const [ga, gb] of gaps) {
      const a = Math.max(start, ga);
      const b = Math.min(end, gb);
      if (b - a <= RULES.freezeMin + 0.01 || b >= recorded - 0.05) continue;
      keepWindow(within(a, b), a, a + RULES.freezeKeep);
      extra.push({ type: "freeze", label: "", from: a, to: b, length: b - a, idx: within(a, b), note: "not marked: look at it" });
    }
  }

  // 3. a person's corrections (out/<scene>.edit.json, "manual")
  for (const m of manual) {
    const [a, b] = m.cut ?? [m.from, m.to];
    const idx = within(a, b);
    for (const i of idx) out[i] = m.cut ? 0 : d[i] / m.speed;
    // (a frame that straddles the edge of a manual range follows its midpoint)
    extra.push({ type: "manual", label: "", from: a, to: b, length: b - a, idx, note: m.cut ? "cut" : `${m.speed}×` });
  }

  // 4. the length budget: holds shortened, the longest first, down to their floors; then a uniform speed-up of at most 1.15×
  const total = () => out.reduce((a, b) => a + b, 0);
  const budget = [];
  const holds = segments.filter((s) => s.type === "hold");
  if (total() > target) {
    for (let guard = 0; guard < 2000 && total() > target; guard++) {
      const length = (s) => s.idx.reduce((sum, i) => sum + out[i], 0);
      const pick = holds
        .map((s) => ({ s, length: length(s), floor: RULES.floors[s.kind] ?? RULES.floors.after }))
        .filter((h) => h.length - h.floor > 0.001)
        .sort((a, b) => b.length - a.length)[0];
      if (!pick) break;
      const step = Math.min(0.1, pick.length - pick.floor, total() - target);
      for (const i of pick.s.idx) out[i] *= (pick.length - step) / pick.length;
      pick.s.note = `shortened to ${fmt(pick.length - step)} s (budget)`;
    }
  }
  if (total() > target) {
    const protectedFrom = recorded - RULES.protectTail;
    const protectedIdx = new Set(segments.filter((s) => s.type === "reveal" || (s.type === "hold" && s.kind === "end")).flatMap((s) => s.idx));
    for (let i = 0; i < frames.length; i++) if (mid[i] >= protectedFrom) protectedIdx.add(i);
    const fixed = [...protectedIdx].reduce((sum, i) => sum + out[i], 0);
    const free = total() - fixed;
    const want = Math.min(RULES.maxSpeedUp, free / Math.max(target - fixed, 0.001));
    if (want > 1.001) {
      for (let i = 0; i < frames.length; i++) if (!protectedIdx.has(i)) out[i] /= want;
      budget.push(`  budget   uniform ${want.toFixed(2)}× (everything but the reveals and the last ${RULES.protectTail} s)`);
    }
  }
  const final = total();
  if (!Number.isFinite(target)) budget.push("  budget   none: the scene has no target (04 §6: the length is a person's edit)");
  else budget.push(final <= target + 0.05 ? `  budget   ok (target ${target} s)` : `  budget   OVER the target of ${target} s by ${fmt(final - target)} s: cut a beat`);

  // the final time of a recorded second
  const prefix = [0];
  for (const x of out) prefix.push(prefix.at(-1) + x);
  const finalAt = (t) => prefix[within(0, t).length]; // frames are in order: the ones before `t` are the first ones
  const all = [...segments, ...extra].sort((a, b) => a.from - b.from).map((s) => {
    const after = s.idx.reduce((sum, i) => sum + out[i], 0);
    return { type: s.type, label: s.label, kind: s.kind, recorded: [s.from, s.to].map((x) => Math.round(x * 100) / 100), final: [finalAt(s.from), finalAt(s.to)].map((x) => Math.round(x * 100) / 100), before: s.length, after, note: s.note };
  });

  const report = [`${label}: recorded ${fmt(recorded)} s → ${fmt(final)} s`];
  for (const s of all) {
    const changed = Math.abs(s.before - s.after) > 0.05;
    if (!changed && !s.note.includes("not marked")) continue;
    report.push(`  ${s.type.padEnd(7)} ${fmt(s.recorded[0]).padStart(5)} – ${fmt(s.recorded[1]).padEnd(5)} ${fmt(s.before)} s → ${fmt(s.after)} s${s.note || s.label ? `   (${[s.label, s.note].filter(Boolean).join(", ")})` : ""}`);
  }
  report.push(...budget);
  return { out, report, recorded, final, segments: all, target };
}

/** Writes `out/<scene>.edit.json`: what was done to each segment, in recorded and in final seconds, and the `manual` list a person can add to. */
export function writeEdit(name, result, manual) {
  const file = join(paths.out, `${name}.edit.json`);
  writeFileSync(file, JSON.stringify({ scene: name, recorded: result.recorded, final: result.final, target: result.target, manual, segments: result.segments }, null, 2));
  return file;
}

/** The GIF (two-pass palette) and the MP4 for posts, from the edited durations. Returns the sizes. */
export function encodeFilm(film, out, { gif, mp4, fps = 12, colors = 128 }) {
  const list = join(paths.out, "frames", film.name, "frames-edit.txt");
  writeList(film, out, list);
  mkdirSync(paths.out, { recursive: true });
  const scale = `scale=${size.width}:${size.height}:flags=lanczos`;
  const filter = `${scale},fps=${fps},split[a][b];[a]palettegen=max_colors=${colors}:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle`;
  execFileSync(FFMPEG, ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", list, "-vf", filter, "-loop", "0", gif], { stdio: "inherit" });
  if (mp4) execFileSync(FFMPEG, ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", list, "-vf", `${scale},fps=30`, "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20", mp4], { stdio: "inherit" });
  return { gifBytes: statSync(gif).size, mp4Bytes: mp4 ? statSync(mp4).size : 0 };
}
