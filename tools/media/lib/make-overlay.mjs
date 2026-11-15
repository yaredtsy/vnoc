// Makes demo/overlay.patch and demo/vnoc/ from demo/docs.mjs, with no window: it clones httpx, starts the core, adds each doc as "Add Doc…" would
// (the core writes the ID line and .vnoc/, this script applies the editor's edit and writes the doc's text), then turns one doc into a Missing one.
//   node lib/make-overlay.mjs [--core <path>] [--driver <path>]     (default: this repository's debug builds)
// The overlay is kept in the repository so every run of the media tool starts from the same IDs. Run this only to change the demo's docs.
import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { here, paths, repoRoot } from "../config.mjs";
import { docs } from "../demo/docs.mjs";
import { CoreClient, uriOf } from "./core-client.mjs";
import { prepareDemo } from "./prepare.mjs";

const arg = (name, fallback) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : fallback);
const corePath = arg("--core", join(repoRoot, "target/debug/vnoc"));
const driverPath = arg("--driver", join(repoRoot, "drivers/python/target/debug/vnoc-driver-python"));

const git = (cwd, ...args) => execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
const root = prepareDemo({ withOverlay: false });
const core = new CoreClient(corePath);
await core.initialize({ driverPath, root });
await core.request("vnoc/structure", null);

let first = true;
for (const doc of docs) {
  const file = doc.key.includes("::") || doc.key.endsWith(".py") ? join(root, doc.key.split("::")[0]) : undefined;
  const params = { key: doc.key, title: doc.title };
  if (first) params.markers = true; // the once-per-project question: write IDs into the code
  if (file) params.document = { uri: uriOf(file), version: 1, text: readFileSync(file, "utf8") };
  const result = await core.request("vnoc/attach", params);
  first = false;
  // the doc is `---\nnode: …\nhint: …\n---\n# Title\n`: the text goes under the heading
  const head = readFileSync(result.doc, "utf8").trimEnd();
  writeFileSync(result.doc, `${head}\n\n${doc.body}\n`);
  console.log(`${doc.key}: ${doc.title} (ID in code: ${result.id_in_code})`);
  if (file && result.id_in_code) await core.filesChanged([file]);
}

// A Missing doc: its function loses its ID line and its name, as if it had been renamed in a tool that does not know V-NOC.
const [gone] = docs.filter((d) => d.missing);
if (gone) {
  const edit = (relative, change) => {
    const path = join(root, relative);
    writeFileSync(path, change(readFileSync(path, "utf8")));
  };
  edit("httpx/_config.py", (text) => text.replace(/^ *"""ID: [0-9A-Za-z]+"""\n/m, "").replaceAll("as_dict", "as_mapping"));
  edit("httpx/_client.py", (text) => text.replaceAll("timeout.as_dict()", "timeout.as_mapping()"));
  await core.filesChanged([join(root, "httpx/_config.py"), join(root, "httpx/_client.py")]);
  const structure = await core.request("vnoc/structure", null);
  if (structure.missing.length !== 1) throw new Error(`expected one Missing doc, the core says ${JSON.stringify(structure.missing)}`);
  console.log(`missing: ${structure.missing[0].title ?? JSON.stringify(structure.missing[0])}`);
}
await core.close();

// the code changes (ID lines, the folder's marker file) go in the patch; .vnoc/ is copied as it is, without local/ (the trash, the user's own files)
git(root, "add", "-A");
writeFileSync(join(here, "demo/overlay.patch"), git(root, "diff", "--cached", "--binary", "--", ".", ":!.vnoc"));
rmSync(join(here, "demo/vnoc"), { recursive: true, force: true });
mkdirSync(join(here, "demo/vnoc"), { recursive: true });
for (const entry of readdirSync(join(root, ".vnoc"))) {
  if (entry !== "local") cpSync(join(root, ".vnoc", entry), join(here, "demo/vnoc", entry), { recursive: true });
}
console.log(`wrote demo/overlay.patch and demo/vnoc/ (work folder: ${paths.work})`);
