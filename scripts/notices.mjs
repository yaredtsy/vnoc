// Writes extension/THIRD_PARTY_NOTICES.md: the licences of the code inside the VSIX that is not ours.
//   - the Rust crates of both Cargo workspaces (the core, and every driver in drivers/: the Python one with ruff, ty and salsa), through `cargo about`
//   - the npm packages the extension and the webview bundle (their production dependencies), through `pnpm licenses`
// Usage: node scripts/notices.mjs [--check]     --check writes nothing and fails if the committed file is not what this would write.
// Needs `cargo install cargo-about --locked --features cli`. The file is committed; scripts/package.mjs fails without it.
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { findDrivers } from "./drivers.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "extension/THIRD_PARTY_NOTICES.md");
const check = process.argv.includes("--check");
const shell = process.platform === "win32";

/** a licence text, trimmed and with line endings made the same, so that equal licences compare equal */
const clean = (text) => text.replace(/\r\n?/g, "\n").trim();

// ── Rust ────────────────────────────────────────────────────────────────────────────────────────────────────────────────

/** crates (and their licence texts) of one Cargo workspace, as `cargo about` reads them. */
function rustLicences(manifest, extra) {
  const dir = mkdtempSync(join(tmpdir(), "vnoc-notices-"));
  const file = join(dir, "about.json");
  try {
    execFileSync("cargo", ["about", "generate", "--format", "json", "--config", join(root, "scripts/notices/about.toml"), "--manifest-path", join(root, manifest), ...extra, "-o", file], {
      cwd: root,
      stdio: ["ignore", "inherit", "inherit"],
      shell,
    });
    return JSON.parse(readFileSync(file, "utf8")).licenses;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** MPL-2.0 asks us to say where the source of the code is. */
const mplNote = (id) => (id.includes("MPL-2.0") ? ["The source code of these crates is available at the links above and on crates.io.", ""] : []);

/** Our own crates are Apache-2.0 and the VSIX carries the LICENSE; they are not "third party". */
const ours = (name) => name.startsWith("vnoc");

function rustSection() {
  const byText = new Map(); // licence text -> { id, name, crates: Map("name version" -> repository) }
  for (const l of [...rustLicences("Cargo.toml", ["--workspace"]), ...findDrivers().flatMap((d) => rustLicences(d.manifest, []))]) {
    const key = clean(l.text);
    const entry = byText.get(key) ?? { id: l.id, name: l.name, crates: new Map() };
    for (const { crate } of l.used_by) if (!ours(crate.name)) entry.crates.set(`${crate.name} ${crate.version}`, crate.repository ?? `https://crates.io/crates/${crate.name}`);
    byText.set(key, entry);
  }
  return [...byText.entries()].filter(([, e]) => e.crates.size > 0).sort(([, a], [, b]) => a.id.localeCompare(b.id) || b.crates.size - a.crates.size);
}

// ── npm ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────

/** The licence file of an installed package: LICENSE, LICENCE or COPYING, in any case and with any extension. */
function licenceFile(dir) {
  const names = readdirSync(dir).filter((f) => /^(license|licence|copying)(\.|-|$)/i.test(f)).sort((a, b) => a.length - b.length || a.localeCompare(b));
  return names.length ? join(dir, names[0]) : undefined;
}

function npmSection() {
  const byText = new Map(); // licence text -> { id, packages: Map("name version" -> homepage) }
  for (const pkg of ["vnoc", "vnoc-webview"]) {
    const listed = JSON.parse(execFileSync("pnpm", ["--filter", pkg, "licenses", "list", "--prod", "--json"], { cwd: root, encoding: "utf8", shell, maxBuffer: 64 << 20 }));
    for (const [id, packages] of Object.entries(listed)) {
      for (const p of packages) {
        if (p.name.startsWith("@vnoc/")) continue;
        for (const [i, dir] of p.paths.entries()) {
          const file = licenceFile(dir);
          if (!file) throw new Error(`${p.name} ${p.versions[i] ?? p.versions[0]}: no licence file in ${dir}`);
          const key = clean(readFileSync(file, "utf8"));
          const entry = byText.get(key) ?? { id, packages: new Map() };
          entry.packages.set(`${p.name} ${p.versions[i] ?? p.versions[0]}`, p.homepage ?? `https://www.npmjs.com/package/${p.name}`);
          byText.set(key, entry);
        }
      }
    }
  }
  return [...byText.entries()].sort(([, a], [, b]) => a.id.localeCompare(b.id) || b.packages.size - a.packages.size);
}

// ── the file ────────────────────────────────────────────────────────────────────────────────────────────────────────────

function render() {
  const lines = [
    "# Third-party notices",
    "",
    "V-NOC is licensed under the Apache License 2.0 (see `LICENSE`). This extension also contains the third-party code listed below, under its own licences.",
    "",
    "- **The V-NOC core and the language drivers** (programs in `bin/`) are written in Rust. The Python driver includes the Python analysis of [ruff](https://github.com/astral-sh/ruff) and [ty](https://github.com/astral-sh/ty) (MIT) and [salsa](https://github.com/salsa-rs/salsa) (Apache-2.0 / MIT), through [language_driver_rs](https://github.com/yaredtsy/language_driver_rs).",
    "- **The extension and its pages** (`dist/`, `media/webview/`) bundle the npm packages listed under *npm packages*.",
    "",
    "This file is written by `node scripts/notices.mjs`; do not edit it by hand.",
    "",
    "## Rust crates",
  ];
  for (const [text, e] of rustSection()) {
    lines.push("", `### ${e.name} (${e.id})`, "", ...mplNote(e.id), "Used by:", "", ...[...e.crates].sort(([a], [b]) => a.localeCompare(b)).map(([name, url]) => `- [${name}](${url})`), "", "```text", text, "```");
  }
  lines.push("", "## npm packages");
  for (const [text, e] of npmSection()) {
    lines.push("", `### ${e.id}`, "", "Used by:", "", ...[...e.packages].sort(([a], [b]) => a.localeCompare(b)).map(([name, url]) => `- [${name}](${url})`), "", "```text", text, "```");
  }
  return lines.join("\n") + "\n";
}

const text = render();
if (check) {
  if (!existsSync(out) || readFileSync(out, "utf8") !== text) {
    console.error("extension/THIRD_PARTY_NOTICES.md is out of date: run `node scripts/notices.mjs` and commit it.");
    process.exit(1);
  }
  console.log("extension/THIRD_PARTY_NOTICES.md is up to date.");
} else {
  writeFileSync(out, text);
  console.log(`wrote ${out}`);
}
