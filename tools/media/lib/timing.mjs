// The rules that turn the marks of a recording and a person's edit list into the time each frame stays on screen (docs/18-release-0.2/04-gifs-v2.md §6.3). Pure: no
// files, no ffmpeg; `edit.mjs` reads the film and the list, finds the freezes, and encodes what this returns. A table test covers it (`test/timing.test.mjs`).
//
// A frame is on screen from its time to the next frame's (the last one until the film ends). Everything below is time taken out of that line:
//   1. the automatic pass, on the recorded line: a `wait` is cut to 0.3 s (its end is kept: the loaded state), a `hold` is trimmed to its kind, a long `typing` plays at 2×,
//      and a stretch that no mark explains and nothing changes in is capped at 0.7 s. No floor, no budget, no uniform speed-up.
//   2. the edit list, on the line the automatic pass made (the review film: it is the film a person watches, so its clock is the one the list uses). Each entry names a beat
//      and times *since the beat began*: `cut` removes a stretch, `speed` plays it faster (the whole beat with no `from`/`to`), `hold` sets how long the beat's last hold stays.

/** The rules of the automatic pass, in one place. */
export const RULES = {
  waitKeep: 0.3, // a wait is cut to this
  holds: { after: 0.6, read: 1.0, end: 3.0 }, // a hold of this kind is trimmed to this (`first` is not trimmed)
  typingLong: 2, // typing longer than this is played faster…
  typingSpeed: 2, // …at this speed
  freezeMin: 0.7, // a stretch of unchanged frames that no mark explains, longer than this, is capped at…
  freezeKeep: 0.7, // …this
};

const EPS = 0.001;
const round = (x) => Math.round(x * 1000) / 1000;
const fmt = (n) => n.toFixed(1);

/** The part of [a, b] that lies inside [c, d]. */
const overlap = (a, b, c, d) => Math.max(0, Math.min(b, d) - Math.max(a, c));

/** `ops`: `{ a, b, f }` plays [a, b] at the fraction `f` of its time (0 removes it). The durations after the ops, for frames that begin at `starts` and last `durations`. */
function applyOps(starts, durations, ops) {
  return durations.map((d, i) => {
    let kept = d;
    for (const op of ops) kept -= overlap(starts[i], starts[i] + d, op.a, op.b) * (1 - op.f);
    return Math.max(0, kept);
  });
}

/** The time of a moment of the line the ops were applied to, on the line after them. */
const afterOps = (t, ops) => t - ops.reduce((sum, op) => sum + overlap(op.a, op.b, -Infinity, t) * (1 - op.f), 0);

/** Merges stretches that touch or overlap. */
function merge(stretches) {
  const out = [];
  for (const s of [...stretches].sort((x, y) => x[0] - y[0])) {
    if (out.length > 0 && s[0] <= out.at(-1)[1] + EPS) out.at(-1)[1] = Math.max(out.at(-1)[1], s[1]);
    else out.push([...s]);
  }
  return out;
}

/**
 * Validates an edit list against the beats of a recording and returns it as entries with their stretch resolved (a, b: seconds on the line the automatic pass made).
 * Throws for anything it cannot do: an unknown beat, an entry that is not exactly one of cut / speed / hold, a stretch outside its beat, two that overlap.
 */
export function resolveEdits(edits, beats, file = "the edit list") {
  if (!Array.isArray(edits)) throw new Error(`${file}: the edit list is a JSON array of entries`);
  const resolved = [];
  for (const [n, entry] of edits.entries()) {
    const at = `${file}, entry ${n + 1}`;
    if (entry === null || typeof entry !== "object" || Array.isArray(entry)) throw new Error(`${at}: an entry is an object like { "beat": "explore", "from": 0, "to": 0.9, "speed": 1.5 }`);
    const known = new Set(["beat", "from", "to", "cut", "speed", "hold"]);
    for (const key of Object.keys(entry)) if (!known.has(key)) throw new Error(`${at}: "${key}" is not a key of an entry (${[...known].join(", ")})`);
    const beat = beats.find((b) => b.name === entry.beat);
    if (!beat) throw new Error(`${at}: the beat "${entry.beat}" is not in the recording (it has ${beats.map((b) => b.name).join(", ")})`);
    const ops = ["cut", "speed", "hold"].filter((k) => k in entry);
    if (ops.length !== 1) throw new Error(`${at}: exactly one of cut, speed and hold (this has ${ops.length === 0 ? "none" : ops.join(" and ")})`);
    const [op] = ops;
    const number = (k, ok, what) => {
      if (typeof entry[k] !== "number" || !Number.isFinite(entry[k]) || !ok(entry[k])) throw new Error(`${at}: "${k}" is ${what}`);
    };
    if (op === "cut" && entry.cut !== true) throw new Error(`${at}: "cut" is true (the stretch is "from" and "to")`);
    if (op === "speed") number("speed", (v) => v > 0, "a number above 0");
    if (op === "hold") {
      number("hold", (v) => v >= 0, "a number of seconds, 0 or more");
      if ("from" in entry || "to" in entry) throw new Error(`${at}: a hold has no "from" or "to"`);
    }
    const length = beat.end - beat.start;
    const from = entry.from ?? 0;
    const to = entry.to ?? length;
    if (op !== "hold") {
      if ("from" in entry) number("from", (v) => v >= 0, "seconds since the beat began, 0 or more");
      if ("to" in entry) number("to", (v) => v > 0, "seconds since the beat began, above 0");
      if (!(from < to)) throw new Error(`${at}: "from" (${from}) is before "to" (${to})`);
      if (to > length + 0.05) throw new Error(`${at}: the stretch ends at ${fmt(to)} s, and the beat "${beat.name}" is ${fmt(length)} s long in the review film`);
    }
    resolved.push({ n: n + 1, beat, op, value: entry[op], a: beat.start + from, b: Math.min(beat.start + to, beat.end), from, to: Math.min(to, length) });
  }
  const stretches = resolved.filter((e) => e.op !== "hold").sort((x, y) => x.a - y.a);
  for (let i = 1; i < stretches.length; i++) {
    if (stretches[i].a < stretches[i - 1].b - EPS) throw new Error(`${file}: entries ${stretches[i - 1].n} and ${stretches[i].n} overlap (${stretches[i - 1].beat.name} and ${stretches[i].beat.name}): one stretch, one change`);
  }
  return resolved;
}

/**
 * The time each frame stays on screen.
 *   film:  `{ times, end, marks, beats, freezes }`. `times`: when each frame appeared (seconds); `end`: when the film ends; `marks`: `{ type, label, kind?, from, to }`;
 *          `beats`: `{ name, t }`; `freezes`: `{ start, end? }` stretches in which the frames did not change (found by ffmpeg; a frame that lasts long is one too).
 *   edits: the edit list as the person wrote it (an array, or none).
 * Returns the durations after the automatic pass (`auto`, what the review film shows) and after the edit list (`durations`, the GIF), the beats on the review film's clock,
 * and `lines`: what the automatic pass did, to print.
 */
export function retime(film, edits = [], { label = "film", file } = {}) {
  const { times, end, marks, beats } = film;
  const recorded = times.map((t, i) => (times[i + 1] ?? end) - t);
  const total = recorded.reduce((a, b) => a + b, 0);
  const gapsOf = (list) => {
    const gaps = [];
    let at = 0;
    for (const m of [...list].sort((x, y) => x.from - y.from)) {
      if (m.from > at) gaps.push([at, m.from]);
      at = Math.max(at, m.to);
    }
    if (at < end) gaps.push([at, end]);
    return gaps;
  };

  // 1. the automatic pass, on the recorded line
  const ops = [];
  const notes = [];
  for (const m of marks) {
    const length = m.to - m.from;
    let op;
    if (m.type === "wait" && length > RULES.waitKeep + EPS) op = { a: m.from, b: m.to - RULES.waitKeep, f: 0, note: `cut to ${RULES.waitKeep} s` };
    else if (m.type === "hold" && length > (RULES.holds[m.kind ?? "after"] ?? Infinity) + EPS) {
      const keep = RULES.holds[m.kind ?? "after"];
      op = { a: m.from + keep, b: m.to, f: 0, note: `${m.kind ?? "after"}, trimmed to ${keep} s` };
    } else if (m.type === "typing" && length > RULES.typingLong) op = { a: m.from, b: m.to, f: 1 / RULES.typingSpeed, note: `${RULES.typingSpeed}×` };
    if (op) {
      ops.push(op);
      notes.push({ type: m.type, label: m.label, from: m.from, to: m.to, note: op.note, ...op });
    }
  }
  // what the marks miss: a stretch in which nothing changes, in the time no mark covers. A frame that lasts long is such a stretch (the screen sent nothing new).
  const still = merge([...(film.freezes ?? []).map((f) => [f.start, f.end ?? end]), ...times.flatMap((t, i) => (recorded[i] > RULES.freezeMin ? [[t, t + recorded[i]]] : []))]);
  for (const [start, stop] of still) {
    for (const [ga, gb] of gapsOf(marks)) {
      const a = Math.max(start, ga);
      const b = Math.min(stop, gb);
      if (b - a <= RULES.freezeMin + 0.01 || b >= end - 0.05) continue;
      const op = { a: a + RULES.freezeKeep, b, f: 0, note: "not marked: look at it" };
      ops.push(op);
      notes.push({ type: "freeze", label: "", from: a, to: b, ...op });
    }
  }
  const auto = applyOps(times, recorded, ops);
  const autoTotal = auto.reduce((a, b) => a + b, 0);

  const report = [`${label}: recorded ${fmt(total)} s → ${fmt(autoTotal)} s (the automatic pass)`];
  for (const n of notes.sort((x, y) => x.from - y.from)) {
    const before = n.to - n.from;
    const after = before - overlap(n.a, n.b, -Infinity, Infinity) * (1 - n.f);
    if (Math.abs(before - after) <= 0.05 && !n.note.includes("not marked")) continue;
    report.push(`  ${n.type.padEnd(7)} ${fmt(n.from).padStart(5)} – ${fmt(n.to).padEnd(5)} ${fmt(before)} s → ${fmt(after)} s   (${[n.label, n.note].filter(Boolean).join(", ")})`);
  }

  // the beats on the review film's clock
  const reviewBeats = [...beats]
    .sort((x, y) => x.t - y.t)
    .map((b, i, all) => ({ name: b.name, t: b.t, tEnd: all[i + 1]?.t ?? end, start: afterOps(b.t, ops), end: i + 1 < all.length ? afterOps(all[i + 1].t, ops) : autoTotal }));

  // 2. the person's edit list, on the line the automatic pass made
  const entries = resolveEdits(edits ?? [], reviewBeats, file);
  const starts = [];
  auto.reduce((at, d, i) => ((starts[i] = at), at + d), 0);
  const stretchOps = [];
  const extra = auto.map(() => 0);
  const done = [];
  for (const e of entries) {
    if (e.op === "cut") stretchOps.push({ a: e.a, b: e.b, f: 0 });
    else if (e.op === "speed") stretchOps.push({ a: e.a, b: e.b, f: 1 / e.value });
    else {
      // the beat's last hold, as the automatic pass left it: on the review film's clock
      const inBeat = marks.filter((m) => m.type === "hold" && m.from >= e.beat.t - EPS && m.from < e.beat.tEnd - EPS);
      const hold = inBeat.sort((x, y) => x.from - y.from).at(-1);
      if (!hold) throw new Error(`${file ?? "the edit list"}, entry ${e.n}: the beat "${e.beat.name}" has no hold to set (the scene did not hold in it)`);
      const a = afterOps(hold.from, ops);
      const b = afterOps(hold.to, ops);
      if (e.value < b - a) stretchOps.push({ a: a + e.value, b, f: 0 });
      else {
        // longer than it was: the frame on screen at its end stays longer
        let last = -1;
        for (let i = 0; i < times.length; i++) if (times[i] < hold.to - EPS) last = i;
        extra[Math.max(last, 0)] += e.value - (b - a);
      }
      e.heldFrom = a;
      e.heldWas = b - a;
    }
    done.push(e);
  }
  const edited = applyOps(starts, auto, stretchOps).map((d, i) => d + extra[i]);
  const editedTotal = edited.reduce((a, b) => a + b, 0);

  const editLines = [];
  if (entries.length > 0) {
    editLines.push(`  the edit list: ${entries.length} ${entries.length === 1 ? "entry" : "entries"}, ${fmt(autoTotal)} s → ${fmt(editedTotal)} s`);
    for (const e of done) {
      const what = e.op === "hold" ? `hold ${fmt(e.heldWas)} s → ${e.value} s` : e.op === "cut" ? `cut ${fmt(e.b - e.a)} s` : `speed ${e.value}× on ${fmt(e.b - e.a)} s`;
      const where = e.op === "hold" ? "" : ` ${fmt(e.from)} – ${fmt(e.to)}`;
      editLines.push(`    ${e.beat.name}${where}: ${what}`);
    }
  }
  return { durations: edited, auto, beats: reviewBeats.map((b) => ({ name: b.name, start: round(b.start), end: round(b.end) })), recorded: total, autoTotal, total: editedTotal, lines: [...report, ...editLines] };
}
