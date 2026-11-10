import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { runTests } from "@vscode/test-electron";

// Runs the suite inside a real VS Code extension host, with a throwaway profile (nothing of the user's is touched), on a copy
// of the domain crate's Python fixture as the opened folder. Needs the core, the driver and the webview built first.
const LOCAL_VSCODE = "/Applications/Visual Studio Code.app/Contents/MacOS/Code";

async function main() {
  const root = path.resolve(__dirname, "../..");
  const repo = path.resolve(root, "..");
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "vnoc-ext-"));
  const workspace = path.join(tmp, "ws");
  fs.mkdirSync(workspace);
  fs.cpSync(path.join(repo, "crates/vnoc-domain/tests/fixtures"), workspace, { recursive: true });
  await runTests({
    // the VS Code on this machine if there is one (or the one named in VSCODE_EXECUTABLE); otherwise the test runner downloads one (CI)
    vscodeExecutablePath: process.env.VSCODE_EXECUTABLE ?? (fs.existsSync(LOCAL_VSCODE) ? LOCAL_VSCODE : undefined),
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
