// Makes the pictures. Usage: node run.mjs [--only g1,g2] [--encode-only g1] [--vsix <file>] [--dev] [--debug] [--publish]
//   --only        the scenes to record, edit and encode (default: all)
//   --encode-only edit and encode again from the saved frames and marks (out/<scene>.frames.json, .marks.json, .edit.json): no VS Code, no recording. A person
//                 corrects the edit in out/<scene>.edit.json ("manual": [{ "from": 6.2, "to": 7.9, "speed": 2 }, { "cut": [3.1, 3.6] }]); delete the file for the automatic edit
//   --dev         load the extension from this repository instead of the VSIX (for writing a scene; the title bar then says Extension Development Host)
//   --debug       also save a screenshot of every beat of a scene to out/debug/
//   --publish     copy what was made from out/ to extension/media/screenshots/ (review every file first: docs/18-release-0.2/01-media.md §8)
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";

import { paths } from "./config.mjs";
import { editFilm, encodeFilm, loadFilm, readManual, writeEdit } from "./lib/edit.mjs";
import { Stage } from "./lib/stage.mjs";

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const value = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);

const scenes = [await import("./lib/scenes/g1-calls.mjs"), await import("./lib/scenes/g2-docs.mjs")];
const encodeOnly = value("--encode-only")?.split(",");
const only = encodeOnly ?? value("--only")?.split(",");
const chosen = scenes.filter((s) => !only || only.includes(s.id));
if (chosen.length === 0) throw new Error(`no scene named ${only}; there are ${scenes.map((s) => s.id).join(", ")}`);

const vsix = value("--vsix") ?? paths.vsix;
if (!encodeOnly && !flag("--dev") && !existsSync(vsix)) throw new Error(`${vsix} does not exist: run \`node scripts/package.mjs\` at the root, or pass --vsix`);
const debug = flag("--debug") ? join(paths.out, "debug") : undefined;
mkdirSync(paths.out, { recursive: true });

for (const scene of chosen) {
  console.log(`\n== ${scene.id} ==`);
  if (!encodeOnly) {
    const stage = await Stage.open({ vsix, dev: flag("--dev"), settings: scene.settings });
    try {
      const { rec, stats } = await scene.run(stage, { debug });
      rec.save();
      console.log(`${stats.frames} frames in ${stats.seconds.toFixed(1)} s (${stats.fps.toFixed(1)} fps)`);
    } catch (error) {
      // what the window showed when the scene failed
      await stage.shot(join(paths.out, "debug", `${scene.id}-failure.png`), { cursor: true }).catch(() => {});
      throw error;
    } finally {
      await stage.close();
    }
  }
  if (scene.kind === "gif") {
    const film = loadFilm(scene.id);
    const manual = readManual(scene.id);
    const result = editFilm(film, { label: scene.id.toUpperCase(), target: scene.target, manual });
    console.log(result.report.join("\n"));
    const edit = writeEdit(scene.id, result, manual);
    const sizes = encodeFilm(film, result.out, { gif: join(paths.out, scene.output), mp4: join(paths.out, scene.output.replace(/\.gif$/, ".mp4")) });
    console.log(`${scene.output}: ${(sizes.gifBytes / 1e6).toFixed(2)} MB (the budget is 6 MB); the edit is in ${edit}`);
  }
}

if (flag("--publish")) {
  mkdirSync(paths.screenshots, { recursive: true });
  for (const scene of chosen) {
    copyFileSync(join(paths.out, scene.output), join(paths.screenshots, scene.output));
    console.log(`published ${scene.output}`);
  }
}
