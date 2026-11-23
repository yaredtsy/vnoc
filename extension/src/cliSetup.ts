import { execFile } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { allowFlag, allowQuestion, type AgentRow, type Changes, decideOffer, detectedNames, errorLine, left, lineOf, type OfferDecision, parseChanges, parseList, parseStatus, rulesFile, setUpArgument, touching } from "./agentsJson.ts";
import { addToPath, cliInstalled, type Facts, linkPath, type Place, planInstall, planMove, planUninstall, parseWrapper, removeFromPath, type Step, wrapperText } from "./cliInstall.ts";
import type { Core } from "./core";

// The command line's two commands and the offer (docs/21-agent-cli/07-install.md §3): *Install `vnoc` Command in PATH*, *Uninstall `vnoc` Command from PATH*, *Set Up Coding Agents*.
// What to make and what to say is `cliInstall.ts` (pure); what the binary's answers mean is `agentsJson.ts` (pure). This file reads the machine, does what they say, and draws the questions.
// It knows no agent, no folder of an agent and no file of V-NOC's `.vnoc/`: the list, the detection and the files are the binary's (P1).

const OFFERED = "vnoc.agentsOffered";
const ADDED_USER_PATH = "vnoc.cli.addedUserPath";
const SET_UP = "Set Up Coding Agents";
const NOT_NOW = "Not now";
const WRITE_THEM = "Write them";

/** What a command that ran the binary got: the exit code and what it printed. */
interface Ran {
  code: number;
  stdout: string;
  stderr: string;
}

/** What *Install* and *Uninstall* answer (the window suite reads it): what was done, and the sentence shown. */
export interface CliResult {
  done: "create" | "move" | "leave" | "remove" | "refuse";
  message: string;
}

/** What *Set Up Coding Agents* answers: the files it wrote and the ones it left. `undefined` when the user stopped at a question. */
export interface SetUpResult {
  wrote: string[];
  left: string[];
}

export class CliSetup {
  /** The last value set for each context key, for the window suite (VS Code cannot be asked for a key it was given). */
  private readonly contexts = new Map<string, boolean>();

  constructor(
    private readonly ctx: vscode.ExtensionContext,
    private readonly core: Core,
    private readonly output: vscode.OutputChannel,
  ) {}

  context(key: string): boolean | undefined {
    return this.contexts.get(key);
  }

  private setContext(key: string, value: boolean): void {
    this.contexts.set(key, value);
    void vscode.commands.executeCommand("setContext", key, value);
  }

  // ───────────────────────────────────────────────────────────────────────────────────────────────────────── the command in PATH

  /** The machine as `cliInstall.ts` wants it. Waits for the core: the command points at the binary the extension runs. */
  private async facts(): Promise<Facts> {
    await this.core.ready;
    const win = process.platform === "win32";
    const base = {
      platform: process.platform,
      home: os.homedir(),
      env: { PATH: process.env.PATH, LOCALAPPDATA: process.env.LOCALAPPDATA, VNOC_CLI_DIR: process.env.VNOC_CLI_DIR, SHELL: process.env.SHELL },
    };
    const where = linkPath(base);
    const dir = "dir" in where ? where.dir : undefined;
    const pkg = this.ctx.extension.packageJSON as { publisher: string; name: string };
    return {
      ...base,
      binary: this.core.binaryPath!,
      drivers: this.core.driverPaths,
      production: this.ctx.extensionMode === vscode.ExtensionMode.Production,
      extension: { publisher: pkg.publisher, name: pkg.name },
      place: "link" in where ? readPlace(where.link, win) : { kind: "nothing" },
      userPath: win ? await readUserPath() : undefined,
      addedToUserPath: win && dir !== undefined && this.ctx.globalState.get(ADDED_USER_PATH) === dir,
    };
  }

  /** Makes or replaces the link (or the wrapper): a temporary one beside it, then a rename, so nobody sees half of it. */
  private place(step: Extract<Step, { do: "create" | "move" }>): void {
    fs.mkdirSync(path.dirname(step.link), { recursive: true });
    const temporary = `${step.link}.vnoc-${process.pid}`;
    fs.rmSync(temporary, { force: true });
    if (process.platform === "win32") fs.writeFileSync(temporary, wrapperText(step.target));
    else fs.symlinkSync(step.target, temporary);
    fs.renameSync(temporary, step.link);
  }

  /** *Install `vnoc` Command in PATH*. */
  async installCli(): Promise<CliResult> {
    const f = await this.facts();
    const plan = planInstall(f);
    if (plan.step.do === "refuse") {
      void vscode.window.showWarningMessage(`V-NOC: ${plan.message}`);
      return { done: "refuse", message: plan.message };
    }
    if (plan.step.do === "create" || plan.step.do === "move") this.place(plan.step);
    if (plan.windowsPath === "add") await this.addToUserPath(linkDir(f));
    void vscode.window.showInformationMessage(`V-NOC: ${plan.message}`);
    this.output.appendLine(`the vnoc command: ${plan.message}`);
    this.setContext("vnoc.cliInstalled", cliInstalled(await this.facts()));
    return { done: plan.step.do, message: plan.message };
  }

  /** *Uninstall `vnoc` Command from PATH*: what the install made, and nothing else. */
  async uninstallCli(): Promise<CliResult> {
    const f = await this.facts();
    const plan = planUninstall(f);
    if (plan.step.do === "remove") {
      fs.unlinkSync(plan.step.link);
      if (plan.windowsPath === "remove") await this.removeFromUserPath(linkDir(f));
      void vscode.window.showInformationMessage(`V-NOC: ${plan.message}`);
    } else {
      void vscode.window.showWarningMessage(`V-NOC: ${plan.message}`);
    }
    this.output.appendLine(`the vnoc command: ${plan.message}`);
    this.setContext("vnoc.cliInstalled", cliInstalled(await this.facts()));
    return { done: plan.step.do, message: plan.message };
  }

  /** On activation: the command is moved to this version when another version's link is there (production only, silently, in the log), and the context keys are set. */
  async activate(): Promise<void> {
    try {
      let f = await this.facts();
      const move = planMove(f);
      if (move?.do === "move") {
        this.place(move);
        this.output.appendLine(`the vnoc command moved from ${move.from} to ${move.target}`);
        f = await this.facts();
      }
      this.setContext("vnoc.cliInstalled", cliInstalled(f));
      await this.refreshAgentsContext();
    } catch (e) {
      this.output.appendLine(`the vnoc command: could not look at it: ${String((e as Error).message ?? e)}`);
    }
  }

  private async addToUserPath(dir: string): Promise<void> {
    const now = await readUserPath();
    await writeUserPath(addToPath(now, dir));
    await this.ctx.globalState.update(ADDED_USER_PATH, dir);
  }

  private async removeFromUserPath(dir: string): Promise<void> {
    await writeUserPath(removeFromPath(await readUserPath(), dir));
    await this.ctx.globalState.update(ADDED_USER_PATH, undefined);
  }

  // ─────────────────────────────────────────────────────────────────────────────────────────────────────────── the coding agents

  /** Runs the binary the extension runs in the workspace folder (it finds the git root from there), and says what it printed. */
  private async runBinary(args: string[]): Promise<Ran> {
    await this.core.ready;
    const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    if (!folder) throw new Error("Open a folder first: V-NOC sets up the agents of the folder you opened.");
    return new Promise((resolve, reject) =>
      execFile(this.core.binaryPath!, args, { cwd: folder, maxBuffer: 16 * 1024 * 1024, windowsHide: true }, (error, stdout, stderr) => {
        if (!error) return resolve({ code: 0, stdout, stderr });
        const code = (error as { code?: unknown }).code;
        // a program that ran and failed has a number; one that could not start has a word (`ENOENT`)
        if (typeof code === "number") return resolve({ code, stdout, stderr });
        reject(new Error(`V-NOC could not run ${this.core.binaryPath}: ${error.message}`));
      }),
    );
  }

  /** A command's answer, or the binary's own `error:` line when it printed none. */
  private async answer(args: string[]): Promise<string> {
    const ran = await this.runBinary(args);
    if (ran.code !== 0 && ran.stdout.trim() === "") throw new Error(errorLine(ran.stderr) ?? `vnoc ${args.slice(0, 2).join(" ")} failed (exit ${ran.code})`);
    return ran.stdout;
  }

  private async changes(agents: string[], dryRun: boolean, allow: boolean): Promise<Changes> {
    return parseChanges(await this.answer(["agent", "install", "--agent", agents.join(","), allowFlag(allow), ...(dryRun ? ["--dry-run"] : []), "--json"]));
  }

  /**
   * The question of the confirmation: let the agents that have a syntax V-NOC knows run its read commands without asking. A one-item list **ticked by
   * default** (a modal has no checkbox); Enter keeps it, Space unticks it, Esc stops. `undefined`: the user stopped. Not asked when no picked agent has one.
   */
  private async askToAllow(names: string[]): Promise<boolean | undefined> {
    if (names.length === 0) return true;
    const item: vscode.QuickPickItem = {
      label: allowQuestion(names),
      detail: "Only the commands that read; writes (doc new, doc tag, relink, repair, init) still ask. Added to your own settings, not committed. Untick it to be asked each time.",
      picked: true,
    };
    const picked = await vscode.window.showQuickPick([item], { canPickMany: true, title: "Set Up Coding Agents: permissions", placeHolder: "Enter to continue with it ticked, Space to untick it, Esc to stop" });
    return picked === undefined ? undefined : picked.length > 0;
  }

  /** `vnoc.agentsSetUp` from `agent status`: V-NOC's files are in this folder. */
  private async refreshAgentsContext(): Promise<boolean> {
    const setUp = parseStatus(await this.answer(["agent", "status", "--json"])).setUp;
    this.setContext("vnoc.agentsSetUp", setUp);
    return setUp;
  }

  /**
   * *Set Up Coding Agents*: the list from the binary, the choice, the files the binary says it would write, the confirmation, the install. `arg` is
   * `{ agents: string[], confirm: true }` to skip both questions (the window suite).
   */
  async setUpAgents(arg?: unknown): Promise<SetUpResult | undefined> {
    const given = setUpArgument(arg);
    const rows = parseList(await this.answer(["agent", "list", "--json"]));
    let agents = given?.agents;
    if (!agents) {
      const picked = await vscode.window.showQuickPick(pickItems(rows), { canPickMany: true, title: "Set Up Coding Agents", placeHolder: "Pick the coding agents the people of this project use" });
      if (!picked) return undefined;
      if (picked.length === 0) {
        void vscode.window.showInformationMessage("V-NOC: Pick at least one agent, or press Esc to stop.");
        return undefined;
      }
      agents = picked.map((p) => p.label);
    }

    // the binary says which of the picked agents it can allow; the question is asked before the files are listed, so the list is what will be written
    let allow = given ? given.permissions : true;
    if (!given) {
      const asked = await this.askToAllow((await this.changes(agents, true, true)).canAllow);
      if (asked === undefined) return undefined;
      allow = asked;
    }

    const plan = await this.changes(agents, true, allow);
    if (touching(plan).length === 0) {
      void vscode.window.showInformationMessage(`V-NOC: Nothing to write: ${agents.join(", ")} ${left(plan).length > 0 ? `has files V-NOC leaves (${left(plan).map(lineOf).join("; ")})` : "already has V-NOC's files"}.`);
      await this.refreshAgentsContext();
      return { wrote: [], left: left(plan).map((a) => a.path) };
    }
    if (!given?.confirm) {
      const choice = await vscode.window.showInformationMessage("V-NOC will write these files:", { modal: true, detail: plan.actions.map((a) => `• ${lineOf(a)}`).join("\n") }, WRITE_THEM);
      if (choice !== WRITE_THEM) return undefined;
    }

    const done = await this.changes(agents, false, allow);
    const wrote = touching(done).map((a) => a.path);
    const kept = left(done).map((a) => a.path);
    const rules = rulesFile(done);
    const text = `V-NOC set up ${agents.join(", ")}: wrote ${wrote.length} file${wrote.length === 1 ? "" : "s"}${kept.length > 0 ? `, left ${kept.join(", ")}` : ""}. Commit them so your team's agents get them.`;
    const open = rules ? `Open ${rules}` : undefined;
    const shown = kept.length > 0 ? vscode.window.showWarningMessage(`V-NOC: ${text}`, ...(open ? [open] : [])) : vscode.window.showInformationMessage(`V-NOC: ${text}`, ...(open ? [open] : []));
    void shown.then((choice) => {
      if (choice && rules) void vscode.window.showTextDocument(vscode.Uri.file(path.join(done.root, rules)));
    });
    this.output.appendLine(`set up ${agents.join(", ")}: wrote ${wrote.join(", ")}`);
    await this.refreshAgentsContext();
    return { wrote, left: kept };
  }

  // ──────────────────────────────────────────────────────────────────────────────────────────────────────────── the offer

  /** What the offer would do now, without showing it: the flag, whether the binary finds an agent in this folder, whether V-NOC's files are there. */
  async decideOffer(): Promise<OfferDecision> {
    const alreadyDecided = this.ctx.workspaceState.get<boolean>(OFFERED) === true;
    if (alreadyDecided) return "decided";
    const detected = detectedNames(parseList(await this.answer(["agent", "list", "--json"])));
    const setUp = detected.length > 0 && parseStatus(await this.answer(["agent", "status", "--json"])).setUp;
    return decideOffer({ alreadyDecided, detected, setUp });
  }

  /** The window suite's way to ask again. */
  async resetOffer(): Promise<void> {
    await this.ctx.workspaceState.update(OFFERED, undefined);
  }

  /**
   * After the first attach in a workspace (07 §3): when somebody here uses a coding agent and V-NOC's files are not there, say so once and offer. The first attach is
   * the only chance, asked or not: the flag is set before anything is shown, so the offer is never made twice in a workspace. Never throws: it is a favour.
   */
  async offerAfterFirstAttach(): Promise<void> {
    try {
      if (this.ctx.workspaceState.get<boolean>(OFFERED) === true) return;
      const decision = await this.decideOffer();
      await this.ctx.workspaceState.update(OFFERED, true);
      if (decision !== "offer") return;
      const choice = await vscode.window.showInformationMessage("V-NOC: Someone here uses a coding agent. Set it up to read V-NOC's notes?", SET_UP, NOT_NOW);
      if (choice === SET_UP) await vscode.commands.executeCommand("vnoc.setUpAgents");
    } catch (e) {
      this.output.appendLine(`the agents offer: ${String((e as Error).message ?? e)}`);
    }
  }
}

// ───────────────────────────────────────────────────────────────────────────────────────────────────────────────── small helpers

/** The agents the QuickPick lists: the binary's names, the detected ones ticked; the files each would get are the rows' own words. */
function pickItems(rows: AgentRow[]): (vscode.QuickPickItem & { picked: boolean })[] {
  return rows.map((r) => ({ label: r.name, description: r.detected ? "found in this project" : "", detail: `skill: ${r.skill} · rules: ${r.rules}`, picked: r.detected }));
}

const linkDir = (f: Facts): string => {
  const where = linkPath(f);
  if ("error" in where) throw new Error(where.error);
  return where.dir;
};

/** What is at `link` now. A link is read, not followed; a `vnoc.cmd` counts only in the shape V-NOC writes. */
function readPlace(link: string, windows: boolean): Place {
  let stat: fs.Stats;
  try {
    stat = fs.lstatSync(link);
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code === "ENOENT" || code === "ENOTDIR") return { kind: "nothing" };
    throw e;
  }
  if (!windows && stat.isSymbolicLink()) {
    const target = path.resolve(path.dirname(link), fs.readlinkSync(link));
    return { kind: "link", target, exists: fs.existsSync(target) };
  }
  if (windows && stat.isFile()) {
    const target = parseWrapper(fs.readFileSync(link, "utf8"));
    if (target) return { kind: "wrapper", target, exists: fs.existsSync(target) };
  }
  return { kind: "file" };
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────── Windows PATH
// `setx` cuts a PATH at 1,024 characters, so the user PATH is read and written where it lives, `HKCU\Environment`, with `reg.exe`, and the change is
// broadcast (WM_SETTINGCHANGE) so new terminals see it. Not run by the suites on macOS and Linux: it is the [check] of 07 §3.

const run = (file: string, args: string[]) =>
  new Promise<string>((resolve, reject) => execFile(file, args, { windowsHide: true }, (error, stdout, stderr) => (error ? reject(new Error(`${file} failed: ${stderr || error.message}`)) : resolve(stdout))));

/** The user PATH as stored, or `undefined` when there is none. */
async function readUserPath(): Promise<string | undefined> {
  if (process.platform !== "win32") return undefined;
  try {
    const out = await run("reg.exe", ["query", "HKCU\\Environment", "/v", "Path"]);
    return out.match(/^\s*Path\s+REG_(?:EXPAND_)?SZ\s+(.*)$/im)?.[1].trimEnd();
  } catch {
    return undefined; // no `Path` value yet: `reg query` says so with an error
  }
}

async function writeUserPath(value: string): Promise<void> {
  await run("reg.exe", ["add", "HKCU\\Environment", "/v", "Path", "/t", "REG_EXPAND_SZ", "/d", value, "/f"]);
  const broadcast =
    "Add-Type -Namespace Vnoc -Name Env -MemberDefinition '[DllImport(\"user32.dll\", SetLastError=true, CharSet=CharSet.Auto)] public static extern IntPtr SendMessageTimeout(IntPtr h, uint m, UIntPtr w, string l, uint f, uint t, out UIntPtr r);';" +
    "$r=[UIntPtr]::Zero; [void][Vnoc.Env]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$r)";
  await run("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", broadcast]);
}

