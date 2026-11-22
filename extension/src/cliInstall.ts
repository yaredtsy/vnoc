import * as nodePath from "node:path";

// *Install `vnoc` Command in PATH* (docs/21-agent-cli/07-install.md §3): what to make, move, leave or refuse, and what to tell the user. **Pure**: no vscode, no file system.
// `cliSetup.ts` reads the facts from the machine, calls these, and does what they say. The same rules serve every platform; the paths use the platform's own separator.
//
// The link points at the core binary the extension itself runs. Only an extension in production moves a link, and only one that points into another version's
// V-NOC extension folder: a development window or the test suite never takes a link it did not make, and a link that points anywhere else is the user's.

/** The extension's identity, from its package.json: the folder VS Code installs it in is `<publisher>.<name>-<version>`. */
export interface Package {
  publisher: string;
  name: string;
}

/** What is at the place where the command goes. `target` is an absolute path; `exists` says whether it is there. */
export type Place =
  | { kind: "nothing" }
  /** a symbolic link (macOS, Linux) */
  | { kind: "link"; target: string; exists: boolean }
  /** a `vnoc.cmd` in the shape V-NOC writes (Windows); `target` is the program it runs */
  | { kind: "wrapper"; target: string; exists: boolean }
  /** anything else: a file or a folder, or a `vnoc.cmd` that is not V-NOC's */
  | { kind: "file" };

export interface Facts {
  platform: string;
  home: string;
  env: { PATH?: string; LOCALAPPDATA?: string; VNOC_CLI_DIR?: string; SHELL?: string };
  /** the core binary the extension runs, as an absolute path (the setting, else `bin/`, else the build folder) */
  binary: string;
  /** the drivers the extension runs: said in the note of a development window, where they are not next to the binary */
  drivers?: string[];
  /** `ExtensionMode.Production`; a development window and the test suite are not */
  production: boolean;
  extension: Package;
  place: Place;
  /** Windows: the user's PATH as stored (`HKCU\Environment`, `Path`), `undefined` when there is none */
  userPath?: string;
  /** Windows: whether an install of ours added the folder to the user PATH (the host remembers it): only then does uninstall take it out */
  addedToUserPath?: boolean;
}

export type Step =
  | { do: "create"; link: string; target: string }
  | { do: "move"; link: string; from: string; target: string }
  | { do: "remove"; link: string; target: string }
  /** nothing to do, and why */
  | { do: "leave"; link: string; why: string }
  /** the place holds something that is not V-NOC's to change: whose, and what the user can do */
  | { do: "refuse"; link: string; why: string };

export interface Plan {
  step: Step;
  /** Windows: the user PATH entry for the folder: add it, it is there already, take it out (uninstall, when install added it), or leave it */
  windowsPath?: "add" | "present" | "remove" | "keep";
  /** macOS, Linux: the folder is not on the PATH: what to add, and where */
  pathAdvice?: string;
  /** a development window: the linked command needs the driver told */
  driverNote?: string;
  /** what to show the user, one paragraph */
  message: string;
}

const isWindows = (f: { platform: string }) => f.platform === "win32";
const pathOf = (f: { platform: string }) => (isWindows(f) ? nodePath.win32 : nodePath.posix);

/** The folder the command goes in, or why there is none. `VNOC_CLI_DIR` wins: for the tests, and for anyone who keeps tools elsewhere. */
export function cliDir(f: Pick<Facts, "platform" | "home" | "env">): { dir: string } | { error: string } {
  const p = pathOf(f);
  const set = f.env.VNOC_CLI_DIR;
  if (set) return { dir: p.resolve(set) };
  if (isWindows(f)) {
    return f.env.LOCALAPPDATA ? { dir: p.join(f.env.LOCALAPPDATA, "vnoc", "bin") } : { error: "LOCALAPPDATA is not set, so there is no folder for the command" };
  }
  return { dir: p.join(f.home, ".local", "bin") };
}

/** The file that is the command: `vnoc` (a link), or `vnoc.cmd` (a wrapper). */
export function linkPath(f: Pick<Facts, "platform" | "home" | "env">): { link: string; dir: string } | { error: string } {
  const where = cliDir(f);
  if ("error" in where) return where;
  return { dir: where.dir, link: pathOf(f).join(where.dir, isWindows(f) ? "vnoc.cmd" : "vnoc") };
}

/** What `vnoc.cmd` holds: it runs the program and hands every argument over. */
export const wrapperText = (binary: string): string => `@"${binary}" %*\r\n`;

/** The program a `vnoc.cmd` runs, when it is in the shape `wrapperText` writes; otherwise `undefined` (it is not ours). */
export function parseWrapper(text: string): string | undefined {
  const line = text.split(/\r?\n/).find((l) => l.trim() !== "");
  return line?.trim().match(/^@"([^"]+)" %\*$/)?.[1];
}

const slashes = (p: string) => p.replace(/\\/g, "/");
const escapeRegExp = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const basename = (p: string) => slashes(p).split("/").pop() ?? "";

/** Whether `target` is the binary of a V-NOC extension folder (`…/<publisher>.<name>-<version>[-<platform>]/bin/vnoc`): some version's, found by name, not hard-coded. */
export function inExtensionFolder(target: string, pkg: Package): boolean {
  return new RegExp(`(^|/)${escapeRegExp(pkg.publisher)}\\.${escapeRegExp(pkg.name)}-\\d[^/]*/bin/vnoc(\\.exe)?$`, "i").test(slashes(target));
}

/** Whether `target` is a V-NOC program at all (by its name): what the context key and uninstall look at. */
export const isVnocBinary = (target: string): boolean => ["vnoc", "vnoc.exe"].includes(basename(target).toLowerCase());

const target = (place: Place): string | undefined => (place.kind === "link" || place.kind === "wrapper" ? place.target : undefined);

/** `vnoc.cliInstalled`: the command exists and points to a V-NOC program that is there. */
export function cliInstalled(f: Pick<Facts, "place">): boolean {
  const p = f.place;
  return (p.kind === "link" || p.kind === "wrapper") && p.exists && isVnocBinary(p.target);
}

// ----------------------------------------------------------------------------------------------------------------------------------- the PATH

/** The entries of a PATH value, as the platform writes them: without a trailing separator, and on Windows without regard to case. */
function entries(value: string | undefined, platform: string): string[] {
  const sep = platform === "win32" ? ";" : ":";
  return (value ?? "")
    .split(sep)
    .map((e) => e.trim().replace(/[\\/]+$/, ""))
    .filter((e) => e !== "")
    .map((e) => (platform === "win32" ? e.toLowerCase() : e));
}

export function onPath(dir: string, value: string | undefined, platform: string): boolean {
  const want = dir.replace(/[\\/]+$/, "");
  return entries(value, platform).includes(platform === "win32" ? want.toLowerCase() : want);
}

/** The user PATH with `dir` added at the end (Windows). Entries already there are kept exactly as they are written. */
export function addToPath(value: string | undefined, dir: string): string {
  const kept = (value ?? "").replace(/;+$/, "");
  return kept === "" ? dir : `${kept};${dir}`;
}

/** The user PATH without `dir` (Windows); every other entry is kept exactly as written. */
export function removeFromPath(value: string | undefined, dir: string): string {
  const want = dir.replace(/[\\/]+$/, "").toLowerCase();
  return (value ?? "")
    .split(";")
    .filter((e) => e.trim() !== "" && e.trim().replace(/[\\/]+$/, "").toLowerCase() !== want)
    .join(";");
}

/** `~/…` for a path under the home folder, as people write it (not on Windows, where nobody writes it so: the path as it is). */
const shown = (p: string, f: Pick<Facts, "platform" | "home">) => {
  if (isWindows(f)) return p;
  const home = f.home.replace(/[\\/]+$/, "");
  return home && slashes(p).startsWith(`${slashes(home)}/`) ? `~/${slashes(p).slice(slashes(home).length + 1)}` : p;
};

/** The command that removes `link`, as the user would type it (Windows: `del`; a path with a space is quoted and, so that the shell takes it, not abbreviated). */
function removeCommand(link: string, f: Pick<Facts, "platform" | "home">): string {
  if (isWindows(f)) return `del "${link}"`;
  const name = shown(link, f);
  return /\s/.test(name) ? `rm "${link}"` : `rm ${name}`;
}

/**
 * The refusal for something at the place that V-NOC did not make. `why` says what it is; the message says how to get rid of it. Install ends with "then run
 * this command again"; uninstall has nothing to run again.
 */
function refusal(f: Pick<Facts, "platform" | "home">, link: string, why: string, again: boolean): { step: Step; message: string } {
  const fix = `If you don't need it: ${removeCommand(link, f)}${again ? ", then run this command again" : ""}.`;
  return { step: { do: "refuse", link, why }, message: `V-NOC left ${shown(link, f)} alone: ${why}. ${fix}` };
}

/** What to say when the folder is not on the PATH (macOS, Linux): the line to add, and to which file, by the shell the environment names. */
export function pathAdvice(f: Pick<Facts, "platform" | "home" | "env">, dir: string): string | undefined {
  if (isWindows(f) || onPath(dir, f.env.PATH, f.platform)) return undefined;
  const name = shown(dir, f);
  const inLine = name.startsWith("~/") ? `$HOME/${name.slice(2)}` : dir;
  const exportLine = `export PATH="${inLine}:$PATH"`;
  const shell = basename(f.env.SHELL ?? "");
  const where =
    shell === "zsh"
      ? `add ${exportLine} to ~/.zshrc`
      : shell === "bash"
        ? `add ${exportLine} to ${f.platform === "darwin" ? "~/.bash_profile" : "~/.bashrc"}`
        : shell === "fish"
          ? `run fish_add_path ${inLine}`
          : `add ${exportLine} to your shell's startup file`;
  return `${name} is not on your PATH: ${where}, then open a new terminal.`;
}

// ------------------------------------------------------------------------------------------------------------------------------------ the plans

const note = (f: Facts): string | undefined =>
  f.production
    ? undefined
    : `This is a development window: the drivers are not next to that binary, so run the command with ${f.drivers?.length ? `VNOC_DRIVERS=${f.drivers.join(pathOf(f).delimiter)}` : "VNOC_DRIVERS set to the drivers' paths"}.`;

/** What *Install `vnoc` Command in PATH* does. */
export function planInstall(f: Facts): Plan {
  const where = linkPath(f);
  if ("error" in where) return { step: { do: "refuse", link: "", why: where.error }, message: `V-NOC could not install the command: ${where.error}.` };
  const { link, dir } = where;
  const have = f.place;
  const refuse = (why: string): Plan => ({ step: { do: "refuse", link, why }, message: `V-NOC left ${shown(link, f)} alone: ${why}` });
  let step: Step;
  if (have.kind === "nothing") {
    step = { do: "create", link, target: f.binary };
  } else if (have.kind === "file") {
    return refusal(f, link, "it is a file V-NOC did not make", true);
  } else if (have.target === f.binary) {
    step = { do: "leave", link, why: `it already runs ${f.binary}` };
  } else if (inExtensionFolder(have.target, f.extension)) {
    if (!f.production) return refuse(`it points into an installed V-NOC (${have.target}), and a development window does not move another one's link.`);
    step = { do: "move", link, from: have.target, target: f.binary };
  } else {
    return refusal(f, link, `it points to ${have.target}, which V-NOC did not make`, true);
  }

  const windowsPath = isWindows(f) ? (onPath(dir, f.userPath, f.platform) ? "present" : "add") : undefined;
  const advice = pathAdvice(f, dir);
  const first =
    step.do === "create"
      ? `Made ${shown(link, f)}, which runs ${f.binary}.`
      : step.do === "move"
        ? `Moved ${shown(link, f)} from ${step.from} to ${f.binary}.`
        : `${shown(link, f)} already runs ${f.binary}.`;
  const parts = [first];
  if (windowsPath === "add") parts.push(`Added ${dir} to your user PATH: open a new terminal.`);
  if (advice) parts.push(advice);
  const driverNote = note(f);
  if (driverNote) parts.push(driverNote);
  return { step, windowsPath, pathAdvice: advice, driverNote, message: parts.join(" ") };
}

/**
 * On activation: the step that moves another version's link to this one, or `undefined`. Only a production extension, and only a link (or our wrapper) that
 * points into another version's V-NOC folder; nothing is ever created here.
 */
export function planMove(f: Facts): Step | undefined {
  const have = f.place;
  if (!f.production || (have.kind !== "link" && have.kind !== "wrapper")) return undefined;
  if (have.target === f.binary || !inExtensionFolder(have.target, f.extension)) return undefined;
  const where = linkPath(f);
  if ("error" in where) return undefined;
  return { do: "move", link: where.link, from: have.target, target: f.binary };
}

/**
 * What *Uninstall `vnoc` Command from PATH* does: takes back what install made, and nothing else. That is a link to a V-NOC extension folder's binary (any
 * version), or to exactly the binary this extension runs (so a development window can undo its own), or our wrapper. A link to any other program called `vnoc`
 * (a person's own build) is theirs. The user PATH entry goes only if install added it.
 */
export function planUninstall(f: Facts): Plan {
  const where = linkPath(f);
  if ("error" in where) return { step: { do: "leave", link: "", why: where.error }, message: `V-NOC could not look for the command: ${where.error}.` };
  const { link, dir } = where;
  const have = f.place;
  if (have.kind === "nothing") return { step: { do: "leave", link, why: "there is no command to remove" }, message: `Nothing to remove: there is no ${shown(link, f)}.` };
  if (have.kind === "file") return refusal(f, link, "it is a file V-NOC did not make", false);
  if (have.target !== f.binary && !inExtensionFolder(have.target, f.extension)) {
    return refusal(f, link, `it points to ${have.target}, which V-NOC did not make`, false);
  }
  const windowsPath = isWindows(f) ? (f.addedToUserPath && onPath(dir, f.userPath, f.platform) ? "remove" : "keep") : undefined;
  const parts = [`Removed ${shown(link, f)}.`];
  if (windowsPath === "remove") parts.push(`Took ${dir} out of your user PATH.`);
  return { step: { do: "remove", link, target: have.target }, windowsPath, message: parts.join(" ") };
}
