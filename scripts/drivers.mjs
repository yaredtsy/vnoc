// The language drivers, found and not listed (docs/23-bend-driver.md §5): every folder `drivers/<x>/` is one, its Cargo.toml's package is `vnoc-driver-<x>`.
// A new language is a new folder; package.mjs and notices.mjs read this and change with no edit. A folder that breaks the rule fails the script.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/** `[{ name: "bend", dir: "<root>/drivers/bend", manifest: "drivers/bend/Cargo.toml", program: "vnoc-driver-bend" }, …]`, by name. */
export function findDrivers() {
  const base = join(root, "drivers");
  const found = [];
  for (const entry of readdirSync(base, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const name = entry.name;
    const manifest = join(base, name, "Cargo.toml");
    if (!existsSync(manifest)) throw new Error(`drivers/${name}/ has no Cargo.toml: every folder in drivers/ is a driver (a package named vnoc-driver-${name})`);
    const pkg = /^\[package\]([\s\S]*?)(?=^\[|(?![\s\S]))/m.exec(readFileSync(manifest, "utf8"))?.[1] ?? "";
    const declared = /^name\s*=\s*"([^"]*)"/m.exec(pkg)?.[1];
    if (declared !== `vnoc-driver-${name}`) throw new Error(`drivers/${name}/Cargo.toml: the package is ${declared ? `"${declared}"` : "not named"}, and must be "vnoc-driver-${name}" (the folder's name)`);
    found.push({ name, dir: join(base, name), manifest: `drivers/${name}/Cargo.toml`, program: `vnoc-driver-${name}` });
  }
  if (found.length === 0) throw new Error("drivers/ holds no driver");
  return found.sort((a, b) => a.name.localeCompare(b.name));
}
