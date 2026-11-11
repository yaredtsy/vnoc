import { execFileSync } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { downloadAndUnzipVSCode, runTests } from "@vscode/test-electron";

// Runs the suite inside a real VS Code extension host, with a throwaway profile (nothing of the user's is touched), on a copy
// of the domain crate's Python fixture as the opened folder. Needs the core, the driver and the webview built first.
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

async function main() {
  const root = path.resolve(__dirname, "../..");
  const repo = path.resolve(root, "..");
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "vnoc-ext-"));
  const workspace = path.join(tmp, "ws");
  fs.mkdirSync(workspace);
  fs.cpSync(path.join(repo, "crates/vnoc-domain/tests/fixtures"), workspace, { recursive: true });
  // a git repository, so the refactor suite can `git mv` (the runners have no git identity: it is given here)
  const git = (...args: string[]) => execFileSync("git", ["-c", "user.name=V-NOC test", "-c", "user.email=test@vnoc.invalid", "-c", "commit.gpgsign=false", ...args], { cwd: workspace, stdio: "ignore" });
  git("init", "-q");
  git("add", "-A");
  git("commit", "-q", "-m", "fixture");
  await runTests({
    vscodeExecutablePath: await vscodeExecutable(),
    extensionDevelopmentPath: root,
    extensionTestsPath: path.join(root, "out/test/suite/index"),
    launchArgs: [workspace, "--disable-extensions", `--user-data-dir=${path.join(tmp, "user")}`, `--extensions-dir=${path.join(tmp, "ext")}`, "--disable-workspace-trust"],
    extensionTestsEnv: { VNOC_TEST_WORKSPACE: workspace },
  });
}
main().catch((e) => {
  console.error(e);
  process.exit(1);
});
