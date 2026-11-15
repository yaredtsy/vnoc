// Makes the pictures. Usage: node run.mjs [--only g1,g2] [--stills] [--encode-only g1] [--vsix <file>] [--dev] [--debug] [--publish]
//   --only        the scenes to record, edit and encode (default: all)
//   --stills      run the scenes without recording: one screenshot per beat and out/<scene>/storyboard.html (docs/18-release-0.2/04-gifs-v2.md §7, gate 1)
//   --encode-only apply the automatic pass and the edit list (tools/media/edits/<scene>.json, written by a person) to the saved frames, marks and beats (out/<scene>.frames.json,
//                 .marks.json, .beats.json) and encode again: no VS Code, no recording. Writes out/<gif>, out/<scene>.review.mp4 (with the clock) and out/review.html
//   --dev         load the extension from this repository instead of the VSIX (for writing a scene; the title bar then says Extension Development Host)
//   --debug       also save a screenshot of every beat of a scene to out/debug/
//   --publish     copy what was made from out/ to extension/media/screenshots/ (review every file first: docs/18-release-0.2/01-media.md §8)
import { copyFileSync, existsSync, mkdirSync, statSync } from "node:fs";
import { join } from "node:path";

import { paths } from "./config.mjs";
import { editFilm, encodeFilm, encodeReview, loadFilm, readEdits } from "./lib/edit.mjs";
import { saveSceneReview, writeReviewPage } from "./lib/review.mjs";
import { Stage } from "./lib/stage.mjs";

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const value = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);

const scenes = [await import("./lib/scenes/g1-calls.mjs"), await import("./lib/scenes/g2-docs.mjs")];
const encodeOnly = value("--encode-only")?.split(",");
const only = encodeOnly ?? value("--only")?.split(",");
const chosen = scenes.filter((s) => !only || only.includes(s.id));
if (chosen.length === 0) throw new Error(`no scene named ${only}; there are ${scenes.map((s) => s.id).join(", ")}`);

const stills = flag("--stills");
if (stills) {
  if (encodeOnly) throw new Error("--stills and --encode-only do not go together: stills run the scene, an encode does not");
  const without = chosen.filter((s) => !s.stills);
  if (without.length > 0) throw new Error(`${without.map((s) => s.id).join(", ")} cannot run with --stills yet (a scene says \`export const stills = true\` when it can)`);
}

const vsix = value("--vsix") ?? paths.vsix;
if (!encodeOnly && !flag("--dev") && !existsSync(vsix)) throw new Error(`${vsix} does not exist: run \`node scripts/package.mjs\` at the root, or pass --vsix`);
const debug = flag("--debug") ? join(paths.out, "debug") : undefined;
mkdirSync(paths.out, { recursive: true });

for (const scene of chosen) {
  console.log(`\n== ${scene.id} ==`);
  if (!encodeOnly) {
    const stage = await Stage.open({ vsix, dev: flag("--dev"), settings: scene.settings });
    try {
      const { rec, stats } = await scene.run(stage, { debug, stills });
      rec.save();
      if (stills) console.log(`${stats.frames} beats; the storyboard is ${rec.file}`);
      else console.log(`${stats.frames} frames in ${stats.seconds.toFixed(1)} s (${stats.fps.toFixed(1)} frames a second; a frame comes when the screen changes); interval median ${stats.medianMs.toFixed(0)} ms, 95 % ${stats.p95Ms.toFixed(0)} ms, longest ${stats.maxGapMs.toFixed(0)} ms, ${stats.over250} over 250 ms`);
    } catch (error) {
      // what the window showed when the scene failed
      await stage.shot(join(paths.out, "debug", `${scene.id}-failure.png`), { cursor: true }).catch(() => {});
      throw error;
    } finally {
      await stage.close();
    }
  }
  if (scene.kind === "gif" && !stills) {
    // the automatic pass, then the person's edit list (edits/<scene>.json; no file, no edits): the GIF; the review film is the automatic pass alone, with its clock
    const film = loadFilm(scene.id);
    const edits = readEdits(scene.id);
    const result = editFilm(film, { label: scene.id.toUpperCase(), entries: edits.entries, file: edits.file });
    console.log(result.lines.join("\n"));
    const gif = join(paths.out, scene.output);
    const sizes = encodeFilm(film, result.durations, { gif, mp4: gif.replace(/\.gif$/, ".mp4") });
    const review = `${scene.id}.review.mp4`;
    encodeReview(film, result.auto, result.beats, join(paths.out, review));
    saveSceneReview({
      id: scene.id, gif: scene.output, review, width: 960, recorded: result.recorded, autoSeconds: result.autoTotal, seconds: result.total, gifBytes: sizes.gifBytes, frames: film.frames.length,
      beats: result.beats, lines: result.lines, editsFile: `tools/media/edits/${scene.id}.json`, editsText: edits.text,
    });
    console.log(`${scene.output}: ${result.total.toFixed(1)} s, ${(sizes.gifBytes / 1e6).toFixed(2)} MB (the budget is 6 MB)`);
  }
}

if (!stills && chosen.some((s) => s.kind === "gif")) console.log(`\nthe review page: ${writeReviewPage(scenes.map((s) => s.id))}`);

if (flag("--publish") && !stills) {
  mkdirSync(paths.screenshots, { recursive: true });
  for (const scene of chosen) {
    copyFileSync(join(paths.out, scene.output), join(paths.screenshots, scene.output));
    console.log(`published ${scene.output}`);
  }
}
