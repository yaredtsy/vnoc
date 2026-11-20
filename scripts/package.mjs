// Build everything and write one platform-specific .vsix, and beside it the command-line archive (docs/21-agent-cli/07-install.md §6):
//   vnoc-<target>.vsix                  the extension, with the two programs inside
//   vnoc-cli-<target>.tar.gz            (win32-x64: .zip) the same two programs, flat, with LICENSE and THIRD_PARTY_NOTICES.md: what scripts/install.sh unpacks
// Usage: node scripts/package.mjs [--target darwin-arm64|darwin-x64|linux-x64|win32-x64] [--pre-release]
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const argTarget = process.argv.indexOf("--target");
const platform = { darwin: "darwin", linux: "linux", win32: "win32" }[process.platform];
const target = argTarget > 0 ? process.argv[argTarget + 1] : `${platform}-${process.arch}`;
// The VSIX target (what vsce and the Marketplace call it) and the Rust target triple the two programs are built for.
const triples = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "win32-x64": "x86_64-pc-windows-msvc",
};
const triple = triples[target];
if (!triple) {
  console.error(`Unknown --target ${target}. Known targets: ${Object.keys(triples).join(", ")}`);
  process.exit(1);
}
// A VSIX is a pre-release because it was packaged as one: `vsce publish --pre-release` does nothing for a file given with `--packagePath`.
const preRelease = process.argv.includes("--pre-release");
// the programs run on the VSIX's system, not on this one: a .exe is a matter of the target (a cross build has the target's names too)
const exe = (name) => (target.startsWith("win32") ? `${name}.exe` : name);

// the third-party licences are committed (node scripts/notices.mjs writes them): a binary that contains other people's code must not ship without them
if (!existsSync(join(root, "extension/THIRD_PARTY_NOTICES.md"))) {
  console.error("extension/THIRD_PARTY_NOTICES.md is missing: run `node scripts/notices.mjs` (it needs `cargo install cargo-about --locked --features cli`) and commit the file.");
  process.exit(1);
}

const run = (cmd, args, cwd = root) => {
  console.log(`\n$ ${cmd} ${args.join(" ")}   (${cwd.replace(root, ".") || "."})`);
  execFileSync(cmd, args, { cwd, stdio: "inherit", shell: process.platform === "win32" });
};

run("cargo", ["build", "--release", "--target", triple, "-p", "vnoc"]);
run("cargo", ["build", "--release", "--target", triple], join(root, "drivers/python"));
run("pnpm", ["--filter", "vnoc-webview", "build"]);
run("pnpm", ["--filter", "vnoc", "build"]);

const bin = join(root, "extension/bin");
mkdirSync(bin, { recursive: true });
for (const f of readdirSync(bin)) if (f !== ".gitkeep") rmSync(join(bin, f));
copyFileSync(join(root, "target", triple, "release", exe("vnoc")), join(bin, exe("vnoc")));
copyFileSync(join(root, "drivers/python/target", triple, "release", exe("vnoc-driver-python")), join(bin, exe("vnoc-driver-python")));

// the licence is one file, at the root of the repository: the VSIX gets a copy (extension/LICENSE is not in git)
copyFileSync(join(root, "LICENSE"), join(root, "extension/LICENSE"));

// The command-line archive: the programs that were just copied into bin/ (nothing is compiled twice), flat, with the two files their licences ask for (the
// programs hold other people's code, as the VSIX does: the check at the top holds here too). Beside the .vsix, at the root; a CI job uploads it as `cli-<target>`.
const archive = join(root, target.startsWith("win32") ? `vnoc-cli-${target}.zip` : `vnoc-cli-${target}.tar.gz`);
const names = [exe("vnoc"), exe("vnoc-driver-python"), "LICENSE", "THIRD_PARTY_NOTICES.md"];
const stage = mkdtempSync(join(tmpdir(), "vnoc-cli-"));
try {
  for (const name of names.slice(0, 2)) {
    copyFileSync(join(bin, name), join(stage, name));
    chmodSync(join(stage, name), 0o755);
  }
  copyFileSync(join(root, "LICENSE"), join(stage, "LICENSE"));
  copyFileSync(join(root, "extension/THIRD_PARTY_NOTICES.md"), join(stage, "THIRD_PARTY_NOTICES.md"));
  rmSync(archive, { force: true });
  // `tar` makes both. On macOS and Linux a .tar.gz; on Windows Windows' own bsdtar (`tar -a` writes a zip by the extension), by its full path: the `tar` first on the PATH
  // of a bash on a runner is GNU tar from Git, which cannot write a zip. COPYFILE_DISABLE keeps macOS from adding `._` files beside each one.
  const tar = process.platform === "win32" ? join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe") : "tar";
  const env = { ...process.env, COPYFILE_DISABLE: "1" };
  const make = process.platform === "win32" ? ["-a", "-c", "-f", archive, "-C", stage, ...names] : ["-czf", archive, "-C", stage, ...names];
  console.log(`\n$ tar ${make.join(" ")}`);
  execFileSync(tar, make, { stdio: "inherit", env });
  execFileSync(tar, [process.platform === "win32" ? "-tvf" : "-tzvf", archive], { stdio: "inherit", env });
} finally {
  rmSync(stage, { recursive: true, force: true });
}

// vsce does not understand pnpm workspaces; the extension is bundled by esbuild, so it needs no node_modules at run time.
run("npx", ["--no", "@vscode/vsce", "package", "--no-dependencies", "--target", target, ...(preRelease ? ["--pre-release"] : []), "--out", join(root, `vnoc-${target}.vsix`)], join(root, "extension"));
