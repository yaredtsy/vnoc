import { join } from "node:path";

// Which program the extension starts for the core or the driver. Pure, so the choice is tested; `extension/src/core.ts` gives it the facts.

export interface BinaryFacts {
  /** what the user set (`vnoc.server.path`, `vnoc.driver.path`); empty when unset */
  configured: string | undefined;
  /** running from source (F5 or the window suite): the extension is not an installed package */
  developing: boolean;
  /** the build folder of this repository */
  fromSource: string;
  /** the copy inside the extension (`bin/`), from the last packaging */
  packaged: string;
  exists(path: string): boolean;
  /** what to say when it is not built: "run `cargo build` in drivers/python" */
  buildHint: string;
  /** "The V-NOC core", "The Python driver" */
  label: string;
}

/**
 * The setting wins. From source the build folder is the only choice: `bin/` may hold a copy from an old packaging run, and falling back to it would run
 * an old program and fail with a message about the wrong thing (a driver from before call trees says the driver has none). Installed, the copy in
 * `bin/` is the only choice.
 */
export function chooseBinary(f: BinaryFacts): { path: string } | { error: string } {
  if (f.configured) return { path: f.configured };
  const path = f.developing ? f.fromSource : f.packaged;
  if (f.exists(path)) return { path };
  return { error: f.developing ? `${f.label} is not built (${path}): ${f.buildHint}` : `${f.label} is not at ${path}` };
}

export interface DriverFacts {
  /** what the user set: `vnoc.drivers`, else `vnoc.driver.path` as a list of one; empty when unset */
  configured: string[];
  /** running from source (F5 or the window suite) */
  developing: boolean;
  /** `<repo>/drivers`: each folder in it is a language, built into `<language>/target/debug` */
  driversDir: string;
  /** the extension's `bin/` */
  binDir: string;
  /** Windows: a driver is `vnoc-driver-<language>.exe` */
  windows: boolean;
  /** the names in a folder; none when it is not there */
  list(dir: string): string[];
}

/** The name rule of the core's `is_driver_file_name`: `vnoc-driver-` and a language of `a-z`, `0-9`, `-`, `_`, then the OS's suffix (so `.d` and `.pdb` are not drivers). */
export function isDriverFileName(name: string, windows: boolean): boolean {
  const rest = name.startsWith("vnoc-driver-") ? name.slice("vnoc-driver-".length) : undefined;
  const language = rest !== undefined && windows ? (rest.endsWith(".exe") ? rest.slice(0, -4) : undefined) : rest;
  return language !== undefined && /^[a-z0-9_-]+$/.test(language);
}

/**
 * Every driver the extension runs, as `chooseBinary` chooses the one core: the setting wins (a list replaces finding); from source every driver built in
 * `drivers/<language>/target/debug`, never `bin/`; installed, every driver in `bin/`. None found is an error that says how to build one.
 */
export function chooseDrivers(f: DriverFacts): { paths: string[] } | { error: string } {
  if (f.configured.length > 0) return { paths: f.configured };
  const folders = f.developing ? f.list(f.driversDir).sort().map((language) => join(f.driversDir, language, "target", "debug")) : [f.binDir];
  const paths = folders.flatMap((folder) => f.list(folder).filter((name) => isDriverFileName(name, f.windows)).sort().map((name) => join(folder, name)));
  if (paths.length > 0) return { paths };
  return {
    error: f.developing
      ? `No driver is built (looked for ${join(f.driversDir, "*", "target", "debug", "vnoc-driver-*")}): build a driver: \`cargo build\` in drivers/<language>`
      : `No driver is in ${f.binDir}`,
  };
}
