import * as assert from "assert";
import { execFileSync } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { guarded } from "./util";

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
/** `real`, but a path that does not exist (a fresh runner has no ~/.local/bin) is returned as it is. */
const realIfThere = (p: string) => (fs.existsSync(p) ? real(p) : p);

/** The folder `runTest.ts` gave the command: a temporary one, empty before each test. */
function cliDir(): string {
  const dir = process.env.VNOC_CLI_DIR;
  assert.ok(dir, "VNOC_CLI_DIR is not set: the suite would install in the real home");
  fs.mkdirSync(dir, { recursive: true });
  assert.ok(real(dir).startsWith(real(os.tmpdir())), `${dir} is not under the temp folder`);
  assert.notStrictEqual(real(dir), realIfThere(path.join(os.homedir(), ".local", "bin")));
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
// With a temporary home folder: `agent remove` and the permissions reach `~/.codex/rules/vnoc.rules`, and a suite must never touch the real one.
const tempHome = fs.mkdtempSync(path.join(os.tmpdir(), "vnoc-cli-home-"));
const vnoc = (args: string[]) => execFileSync(api.cli.binaryPath()!, args, { cwd: workspace(), encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], env: { ...process.env, HOME: tempHome, USERPROFILE: tempHome } });

suite("cli", () => {
  suiteSetup(async () => {
    api = guarded(await vscode.extensions.getExtension("vnoc.vnoc")!.activate());
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
      // a window that is not production says the drivers are not beside the binary
      assert.ok(made.message.includes("VNOC_DRIVERS"), made.message);
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
      assert.ok(file.message.includes("a file V-NOC did not make"), file.message);
      assert.ok(file.message.includes(process.platform === "win32" ? `del "${link}"` : "If you don't need it: rm "), file.message);
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

  test("Uninstall leaves a link to a program that is only named vnoc, and says what to type", async function () {
    if (process.platform === "win32") return this.skip(); // links are not made on Windows
    const dir = cliDir();
    clean(dir);
    const link = path.join(dir, "vnoc");
    const mine = fs.mkdtempSync(path.join(os.tmpdir(), "vnoc-cli-mine-"));
    try {
      // a person's own build: called vnoc, runs, and is not a V-NOC extension's binary nor the one this window runs
      const theirs = path.join(mine, "vnoc");
      fs.writeFileSync(theirs, "#!/bin/sh\necho vnoc of my own\n", { mode: 0o755 });
      fs.symlinkSync(theirs, link);
      const left = (await vscode.commands.executeCommand("vnoc.uninstallCli")) as CliResult;
      assert.strictEqual(left.done, "refuse", left.message);
      assert.ok(left.message.includes(`points to ${theirs}, which V-NOC did not make. If you don't need it: rm `), left.message);
      assert.ok(!left.message.includes("run this command again"), left.message);
      assert.strictEqual(fs.readlinkSync(link), theirs, "the link is as it was");
      assert.ok(fs.existsSync(theirs));
      // the command exists, so the context key says so: it only says the command is there
      assert.strictEqual(api.cli.context("vnoc.cliInstalled"), true);
    } finally {
      clean(dir);
      fs.rmSync(mine, { recursive: true, force: true });
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
      const dry = JSON.parse(vnoc(["agent", "install", "--agent", "claude", "--permissions", "--dry-run", "--json"])) as { actions: { path: string; change: string }[] };
      const listed = dry.actions.filter((a) => a.change !== "same" && a.change !== "leave").map((a) => a.path);
      assert.ok(listed.length >= 2, `the dry run lists the skill and the rules: ${listed}`);
      assert.ok(listed.includes(".claude/settings.local.json"), `and the permissions, which the command allows unless told not to: ${listed}`);
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

  test("Set Up Coding Agents writes the permissions when permissions is true (the default) and none when it is false", async () => {
    const root = workspace();
    const settings = path.join(root, ".claude", "settings.local.json");
    const cleanUp = () => {
      try {
        vnoc(["agent", "remove", "--agent", "claude"]);
      } catch {
        /* nothing was written */
      }
      ["AGENTS.md", "CLAUDE.md", ".claude"].forEach((name) => fs.rmSync(path.join(root, name), { recursive: true, force: true }));
    };
    const entries = (): string[] => (JSON.parse(fs.readFileSync(settings, "utf8")) as { permissions: { allow: string[] } }).permissions.allow;
    cleanUp();
    try {
      // false: the files, and no settings file
      const declined = (await vscode.commands.executeCommand("vnoc.setUpAgents", { agents: ["claude"], confirm: true, permissions: false })) as SetUpResult;
      assert.ok(!declined.wrote.includes(".claude/settings.local.json"), String(declined.wrote));
      assert.ok(!fs.existsSync(settings), "no settings file was written");
      cleanUp();

      // true, and the default: V-NOC's read commands, and never a write
      const given = (await vscode.commands.executeCommand("vnoc.setUpAgents", { agents: ["claude"], confirm: true, permissions: true })) as SetUpResult;
      assert.ok(given.wrote.includes(".claude/settings.local.json"), String(given.wrote));
      // V-NOC's read commands, and the tour commands an agent writes a tour with: how many there are is the core's list (it grew with the tours), not this test's
      const count = entries().length;
      assert.ok(entries().includes("Bash(vnoc show *)") && entries().includes("Bash(vnoc skill *)") && count >= 15 && entries().every((e) => e.startsWith("Bash(vnoc ")), String(entries()));
      assert.ok(!entries().some((e) => /agent|doc new|doc tag|relink|repair|init/.test(e)), "no write command is allowed");
      cleanUp();
      const byDefault = (await vscode.commands.executeCommand("vnoc.setUpAgents", { agents: ["claude"], confirm: true })) as SetUpResult;
      assert.ok(byDefault.wrote.includes(".claude/settings.local.json"), String(byDefault.wrote));
      assert.strictEqual(entries().length, count);

      // the user's own entry stays when the setup runs again over it
      fs.writeFileSync(settings, JSON.stringify({ permissions: { allow: ["Bash(ls *)"] } }, null, 2));
      await vscode.commands.executeCommand("vnoc.setUpAgents", { agents: ["claude"], confirm: true });
      assert.strictEqual(entries()[0], "Bash(ls *)");
      assert.strictEqual(entries().length, count + 1);
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
