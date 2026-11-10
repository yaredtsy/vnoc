// Build everything and write one platform-specific .vsix.  Usage: node scripts/package.mjs [--target darwin-arm64]
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const argTarget = process.argv.indexOf("--target");
const platform = { darwin: "darwin", linux: "linux", win32: "win32" }[process.platform];
const target = argTarget > 0 ? process.argv[argTarget + 1] : `${platform}-${process.arch}`;
const exe = (name) => (process.platform === "win32" ? `${name}.exe` : name);

const run = (cmd, args, cwd = root) => {
  console.log(`\n$ ${cmd} ${args.join(" ")}   (${cwd.replace(root, ".") || "."})`);
  execFileSync(cmd, args, { cwd, stdio: "inherit", shell: process.platform === "win32" });
};

run("cargo", ["build", "--release", "-p", "vnoc"]);
run("cargo", ["build", "--release"], join(root, "drivers/python"));
run("npm", ["run", "build", "-w", "webview"]);
run("npm", ["run", "build", "-w", "extension"]);

const bin = join(root, "extension/bin");
mkdirSync(bin, { recursive: true });
for (const f of readdirSync(bin)) if (f !== ".gitkeep") rmSync(join(bin, f));
copyFileSync(join(root, "target/release", exe("vnoc")), join(bin, exe("vnoc")));
copyFileSync(join(root, "drivers/python/target/release", exe("vnoc-driver-python")), join(bin, exe("vnoc-driver-python")));

// vsce does not understand npm workspaces; the extension is bundled by esbuild, so it needs no node_modules at run time.
run("npx", ["--no", "@vscode/vsce", "package", "--no-dependencies", "--target", target, "--allow-missing-repository", "--skip-license", "--out", join(root, `vnoc-${target}.vsix`)], join(root, "extension"));
