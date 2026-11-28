import { execFileSync } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { downloadAndUnzipVSCode, runTests } from "@vscode/test-electron";

// Runs the suite inside a real VS Code extension host, with a throwaway profile (nothing of the user's is touched), on a copy
// of the domain crate's Python fixture, and a small Bend project in `bend/`, as the opened folder. Needs the core, the driver and the webview built first.
const LOCAL_VSCODE = "/Applications/Visual Studio Code.app/Contents/MacOS/Code";

/**
 * The VS Code the suite runs in: the one named in VSCODE_EXECUTABLE, else the one on this machine, else a download (CI).
 * The downloaded macOS app names its program `Contents/MacOS/Code` since VS Code 1.14x, and a test-electron that does not know says `MacOS/Electron`: use the one that exists.
 */
async function vscodeExecutable(): Promise<string> {
  if (process.env.VSCODE_EXECUTABLE) return process.env.VSCODE_EXECUTABLE;
  if (fs.existsSync(LOCAL_VSCODE)) return LOCAL_VSCODE;
  const downloaded = await downloadAndUnzipVSCode();
  if (fs.existsSync(downloaded)) return downloaded;
  const electron = path.join("MacOS", "Electron");
  if (downloaded.endsWith(electron)) {
    const code = path.join(path.dirname(downloaded), "Code");
    if (fs.existsSync(code)) return code;
  }
  throw new Error(`The downloaded VS Code has no program at ${downloaded}${downloaded.endsWith(electron) ? " or beside it as Code" : ""}.`);
}

/** Every `.py` and `.md` file under `dir` with each "\n" that is not already part of "\r\n" turned into "\r\n". */
function crlfAll(dir: string): void {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const file = path.join(dir, entry.name);
    if (entry.isDirectory()) crlfAll(file);
    else if (/\.(py|md)$/.test(entry.name)) fs.writeFileSync(file, fs.readFileSync(file, "utf8").replace(/(?<!\r)\n/g, "\r\n"));
  }
}

/** A copy of the fixtures, as a git repository: one window's folder. */
function makeWorkspace(tmp: string, name: string): string {
  const root = path.resolve(__dirname, "../..");
  const repo = path.resolve(root, "..");
  const workspace = path.join(tmp, name);
  fs.mkdirSync(workspace);
  fs.cpSync(path.join(repo, "crates/vnoc-domain/tests/fixtures"), workspace, { recursive: true });
  // a small project in a second language, beside the first (its own folder: the domain crate's fixtures are golden-tested and must not change)
  fs.cpSync(path.join(root, "src/test/fixtures/bend"), path.join(workspace, "bend"), { recursive: true });
  // on Windows git checks the fixtures out with CRLF (there is no .gitattributes, and the CRLF run of Save on a .py is wanted); VNOC_FIXTURE_EOL=crlf gives the suite that checkout anywhere
  if (process.env.VNOC_FIXTURE_EOL === "crlf") crlfAll(workspace);
  // a git repository, so the refactor suite can `git mv` (the runners have no git identity: it is given here)
  const git = (...args: string[]) => execFileSync("git", ["-c", "user.name=V-NOC test", "-c", "user.email=test@vnoc.invalid", "-c", "commit.gpgsign=false", ...args], { cwd: workspace, stdio: "ignore" });
  git("init", "-q");
  // the identity is saved in the repository too: the core's own `git commit` (Commit and Pin) has none of the `-c` flags above
  git("config", "user.name", "V-NOC test");
  git("config", "user.email", "test@vnoc.invalid");
  git("config", "commit.gpgsign", "false");
  git("add", "-A");
  git("commit", "-q", "-m", "fixture");
  return workspace;
}

/**
 * A window that another application covers (or that is in the background) is a hidden page to Chromium: a webview in it gets no animation frames, React Flow's fit never ends, and a reveal that waits for it
 * never does (the suites' "timed out waiting for the canvas to do the reveal"). The suite is run from a terminal, with other work going on, so these windows are not throttled.
 */
const NO_THROTTLING = ["--disable-backgrounding-occluded-windows", "--disable-renderer-backgrounding", "--disable-features=CalculateNativeWinOcclusion"];

/** The suites that run in a window of their own, with a core whose first build is held past the time a request may wait, and whose count of a tour's calls never ends (`suite/waits.test.ts`). */
const HELD = ["waits"];

/** One window, one run of its suites. `env` is for the suites and, through the extension, for the core. */
async function window(tmp: string, name: string, env: Record<string, string>): Promise<void> {
  const root = path.resolve(__dirname, "../..");
  const workspace = makeWorkspace(tmp, `ws-${name}`);
  const userData = path.join(tmp, `user-${name}`);
  await runTests({
    vscodeExecutablePath: await vscodeExecutable(),
    extensionDevelopmentPath: root,
    extensionTestsPath: path.join(root, "out/test/suite/index"),
    launchArgs: [workspace, ...NO_THROTTLING, "--disable-extensions", `--user-data-dir=${userData}`, `--extensions-dir=${path.join(tmp, `ext-${name}`)}`, "--disable-workspace-trust", "--force-prefers-reduced-motion"],
    // the `cli` suite installs the `vnoc` command: into this folder, never the real ~/.local/bin (and nothing of the user's PATH)
    extensionTestsEnv: { VNOC_TEST_WORKSPACE: workspace, VNOC_CLI_DIR: path.join(tmp, `cli-bin-${name}`), ...env },
  });
}

async function main() {
  delete process.env.ELECTRON_RUN_AS_NODE; // a terminal inside an Electron app sets it; VS Code then runs the workspace folder as a script
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "vnoc-ext-"));
  const asked = (process.env.VNOC_SUITE ?? "").split(",").map((s) => s.trim()).filter(Boolean);
  const main = asked.length === 0 || asked.some((s) => !HELD.includes(s));
  const held = asked.length === 0 || asked.some((s) => HELD.includes(s));
  const failed: unknown[] = [];
  // the ordinary window first (all its suites); then the one whose core is held. A failure in the first does not stop the second
  if (main) await window(tmp, "main", {}).catch((e) => failed.push(e));
  if (held) await window(tmp, "held", { VNOC_WINDOW: "held", VNOC_WAIT_LIMIT_MS: "2000", VNOC_HOLD_FIRST_BUILD_MS: "12000", VNOC_HOLD_TOUR_CALLS_MS: "900000" }).catch((e) => failed.push(e));
  if (failed.length > 0) throw failed[0];
}
main().catch((e) => {
  console.error(e);
  process.exit(1);
});
