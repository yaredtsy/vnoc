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
