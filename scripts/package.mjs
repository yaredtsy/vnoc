// Build everything and write one platform-specific .vsix.  Usage: node scripts/package.mjs [--target darwin-arm64|darwin-x64|linux-x64|win32-x64] [--pre-release]
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readdirSync, rmSync } from "node:fs";
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

// vsce does not understand pnpm workspaces; the extension is bundled by esbuild, so it needs no node_modules at run time.
run("npx", ["--no", "@vscode/vsce", "package", "--no-dependencies", "--target", target, "--allow-missing-repository", "--skip-license", ...(preRelease ? ["--pre-release"] : []), "--out", join(root, `vnoc-${target}.vsix`)], join(root, "extension"));
