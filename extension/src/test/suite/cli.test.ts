import * as assert from "assert";
import { execFileSync } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

// The command line's two commands in a real window (docs/21-agent-cli/07-install.md §3): *Install `vnoc` Command in PATH* with `VNOC_CLI_DIR` set to a temporary folder (`runTest.ts`
// sets it: **never the real ~/.local/bin**, and on Windows, where an install writes the user PATH in the registry, only the refusal of a foreign file is run), and *Set Up Coding
// Agents* in the suite's temporary workspace. What the pure parts decide is `cliInstall.test.ts` and `agentsJson.test.ts`; here it is that the commands do it, with the binary this
// window runs.

type Offer = "offer" | "decided" | "no-agent-here" | "set-up";
interface Api {
  cli: { binaryPath(): string | undefined; context(key: string): boolean | undefined; decideOffer(): Promise<Offer>; resetOffer(): Promise<void>; offerAfterFirstAttach(): Promise<void> };
}
interface CliResult {
  done: string;
  message: string;
}
interface SetUpResult {
  wrote: string[];
  left: string[];
}
let api: Api;

const workspace = () => vscode.workspace.workspaceFolders![0].uri.fsPath;
const real = (p: string) => fs.realpathSync(p);

/** The folder `runTest.ts` gave the command: a temporary one, empty before each test. */
function cliDir(): string {
  const dir = process.env.VNOC_CLI_DIR;
  assert.ok(dir, "VNOC_CLI_DIR is not set: the suite would install in the real home");
  fs.mkdirSync(dir, { recursive: true });
  assert.ok(real(dir).startsWith(real(os.tmpdir())), `${dir} is not under the temp folder`);
  assert.notStrictEqual(real(dir), real(path.join(os.homedir(), ".local", "bin")));
  return dir;
}
const clean = (dir: string) => fs.readdirSync(dir).forEach((name) => fs.rmSync(path.join(dir, name), { recursive: true, force: true }));
const exists = (p: string) => fs.existsSync(p) || (() => { try { return fs.lstatSync(p).isSymbolicLink(); } catch { return false; } })();

/** Every file under `dir`, relative, without `.git`. */
function files(dir: string, base = dir): string[] {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    if (e.name === ".git") return [];
    const full = path.join(dir, e.name);
    return e.isDirectory() ? files(full, base) : [path.relative(base, full).split(path.sep).join("/")];
  });
}

/** The binary this window runs, run by hand: what the dry run says it would write. */
const vnoc = (args: string[]) => execFileSync(api.cli.binaryPath()!, args, { cwd: workspace(), encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });

suite("cli", () => {
  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    await api.cli.binaryPath(); // the core has started: the binary is known
    assert.ok(api.cli.binaryPath(), "the extension knows the binary it runs");
  });

  test("Install makes the link to the binary this window runs, the context key follows, and Uninstall takes back exactly that", async function () {
    if (process.platform === "win32") return this.skip(); // an install there writes the user PATH in the registry: not from a suite
    const dir = cliDir();
    clean(dir);
    const link = path.join(dir, "vnoc");
    try {
      const made = (await vscode.commands.executeCommand("vnoc.installCli")) as CliResult;
      assert.strictEqual(made.done, "create", made.message);
      assert.ok(fs.lstatSync(link).isSymbolicLink());
      assert.strictEqual(real(link), real(api.cli.binaryPath()!), "the link runs the binary the extension runs");
      assert.ok(path.isAbsolute(fs.readlinkSync(link)), "the target is an absolute path");
      assert.strictEqual(api.cli.context("vnoc.cliInstalled"), true);
      // a window that is not production says the driver is not beside the binary
      assert.ok(made.message.includes("VNOC_DRIVER"), made.message);
      // the command works from where it was made
      assert.ok(execFileSync(link, ["--version"], { encoding: "utf8" }).startsWith("vnoc "));
      // install again: nothing to do, and it says so
      assert.strictEqual(((await vscode.commands.executeCommand("vnoc.installCli")) as CliResult).done, "leave");

      const removed = (await vscode.commands.executeCommand("vnoc.uninstallCli")) as CliResult;
      assert.strictEqual(removed.done, "remove", removed.message);
      assert.ok(!exists(link));
      assert.strictEqual(api.cli.context("vnoc.cliInstalled"), false);
      assert.strictEqual(((await vscode.commands.executeCommand("vnoc.uninstallCli")) as CliResult).done, "leave");
    } finally {
      clean(dir);
    }
  });

  test("a foreign file, a foreign link, and another version's link in a window that is not production are refused and left as they were", async function () {
    const dir = cliDir();
    clean(dir);
    const link = path.join(dir, process.platform === "win32" ? "vnoc.cmd" : "vnoc");
    try {
      fs.writeFileSync(link, "mine\n");
      const file = (await vscode.commands.executeCommand("vnoc.installCli")) as CliResult;
      assert.strictEqual(file.done, "refuse", file.message);
      assert.ok(file.message.includes("it is yours"), file.message);
      assert.strictEqual(fs.readFileSync(link, "utf8"), "mine\n");
      assert.strictEqual(((await vscode.commands.executeCommand("vnoc.uninstallCli")) as CliResult).done, "refuse");
      assert.strictEqual(fs.readFileSync(link, "utf8"), "mine\n");
      assert.notStrictEqual(api.cli.context("vnoc.cliInstalled"), true);
      if (process.platform === "win32") return; // links are not made on Windows

      // a link to something that is not V-NOC's
      fs.rmSync(link);
      fs.symlinkSync(process.execPath, link);
      assert.strictEqual(((await vscode.commands.executeCommand("vnoc.installCli")) as CliResult).done, "refuse");
      assert.strictEqual(((await vscode.commands.executeCommand("vnoc.uninstallCli")) as CliResult).done, "refuse");
      assert.strictEqual(fs.readlinkSync(link), process.execPath);

      // another version's link: only a production extension moves it
      fs.rmSync(link);
      const other = path.join(os.tmpdir(), "vnoc-cli-suite", "extensions", "vnoc.vnoc-0.0.1", "bin", "vnoc");
      fs.symlinkSync(other, link);
      const moved = (await vscode.commands.executeCommand("vnoc.installCli")) as CliResult;
      assert.strictEqual(moved.done, "refuse", moved.message);
      assert.ok(moved.message.includes("development window"), moved.message);
      assert.strictEqual(fs.readlinkSync(link), other, "the link was not moved");
    } finally {
      clean(dir);
    }
  });

  test("Set Up Coding Agents with { agents, confirm } writes exactly the files the dry run listed, and the context key follows", async () => {
    const root = workspace();
    const added = ["AGENTS.md", "CLAUDE.md", ".claude", ".agents"];
    const cleanUp = () => {
      try {
        vnoc(["agent", "remove"]);
      } catch {
        /* nothing was written */
      }
      added.forEach((name) => fs.rmSync(path.join(root, name), { recursive: true, force: true }));
    };
    cleanUp();
    try {
      const before = files(root);
      // what the binary says it would write, asked of it directly
      const dry = JSON.parse(vnoc(["agent", "install", "--agent", "claude", "--dry-run", "--json"])) as { actions: { path: string; change: string }[] };
      const listed = dry.actions.filter((a) => a.change !== "same" && a.change !== "leave").map((a) => a.path);
      assert.ok(listed.length >= 2, `the dry run lists the skill and the rules: ${listed}`);
      assert.deepStrictEqual(files(root), before, "a dry run writes nothing");

      await api.cli.resetOffer();
      assert.strictEqual(await api.cli.decideOffer(), "no-agent-here", "no agent leaves a sign in the fixture");
      assert.notStrictEqual(api.cli.context("vnoc.agentsSetUp"), true);

      const done = (await vscode.commands.executeCommand("vnoc.setUpAgents", { agents: ["claude"], confirm: true })) as SetUpResult;
      assert.deepStrictEqual([...done.wrote].sort(), [...listed].sort(), "the command wrote what the dry run listed");
      assert.deepStrictEqual(done.left, []);
      assert.deepStrictEqual(files(root).filter((f) => !before.includes(f)).sort(), [...listed].sort(), "and exactly those files are new");
      assert.strictEqual(api.cli.context("vnoc.agentsSetUp"), true);
      // V-NOC's files are there: the offer has nothing to offer
      assert.strictEqual(await api.cli.decideOffer(), "set-up");

      // again: nothing to write, nothing changes
      const again = (await vscode.commands.executeCommand("vnoc.setUpAgents", { agents: ["claude"], confirm: true })) as SetUpResult;
      assert.deepStrictEqual(again.wrote, []);
    } finally {
      cleanUp();
    }
  });

  test("a name the binary does not know shows the binary's own error line, and writes nothing", async () => {
    const root = workspace();
    const before = files(root);
    // the command's wrapper shows the binary's `error:` line as a message and answers nothing
    const result = await vscode.commands.executeCommand("vnoc.setUpAgents", { agents: ["no-such-agent"], confirm: true });
    assert.strictEqual(result, undefined, "a failed command answers nothing");
    assert.deepStrictEqual(files(root), before);
  });

  test("the offer is decided once per workspace: the first attach settles it, asked or not", async () => {
    const root = workspace();
    const agentsMd = path.join(root, "AGENTS.md");
    await api.cli.resetOffer();
    try {
      assert.strictEqual(await api.cli.decideOffer(), "no-agent-here");
      // the first attach settles it even when there was nothing to offer (nothing is shown here: no agent leaves a sign): never asked again in this workspace
      await api.cli.offerAfterFirstAttach();
      assert.strictEqual(await api.cli.decideOffer(), "decided");
      await api.cli.resetOffer();
      fs.writeFileSync(agentsMd, "# Somebody here uses an agent\n");
      assert.strictEqual(await api.cli.decideOffer(), "offer", "an AGENTS.md is the sign, and V-NOC's files are not there");
      await vscode.commands.executeCommand("vnoc.setUpAgents", { agents: ["claude"], confirm: true });
      assert.strictEqual(await api.cli.decideOffer(), "set-up");
    } finally {
      try {
        vnoc(["agent", "remove"]);
      } catch {
        /* nothing to remove */
      }
      ["AGENTS.md", "CLAUDE.md", ".claude", ".agents"].forEach((name) => fs.rmSync(path.join(root, name), { recursive: true, force: true }));
      await api.cli.resetOffer();
    }
  });
});
