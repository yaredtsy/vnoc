import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { allowFlag, allowQuestion, decideOffer, detectedNames, errorLine, left, lineOf, parseChanges, parseList, parseStatus, rulesFile, setUpArgument, touching } from "./agentsJson.ts";

// The fixtures are what the real binary printed (`agent list --json`, `agent install --agent claude,codex [--dry-run] --json`, `agent status --json`) on a project
// with only an AGENTS.md; the window suite runs the same commands for real.
const fixture = (name: string) => readFileSync(new URL(`./fixtures/${name}.json`, import.meta.url), "utf8");

test("agent list: every name the binary gives, with its skill file and rules file, and which are detected", () => {
  const rows = parseList(fixture("agent-list"));
  assert.equal(rows.length, 12);
  assert.deepEqual(rows[0], { name: "claude", detected: false, skill: ".claude/skills/vnoc/SKILL.md", rules: "AGENTS.md" });
  assert.ok(rows.some((r) => r.name === "other"));
  // AGENTS.md alone is a sign of `other`, and only that
  assert.deepEqual(detectedNames(rows), ["other"]);
});

test("agent install --dry-run: the files with what happens to each, and the lines for the agents' permissions", () => {
  const changes = parseChanges(fixture("agent-install-dry-run"));
  assert.equal(changes.dryRun, true);
  assert.equal(changes.root, "/path/to/project");
  assert.deepEqual(changes.agents, ["claude", "codex"]);
  assert.deepEqual(
    changes.actions.map((a) => [a.part, a.path, a.change]),
    [
      ["skill", ".claude/skills/vnoc/SKILL.md", "create"],
      ["skill", ".agents/skills/vnoc/SKILL.md", "create"],
      ["rules", "AGENTS.md", "add_block"],
      ["import", "CLAUDE.md", "create"],
    ],
  );
  assert.equal(touching(changes).length, 4);
  assert.deepEqual(left(changes), []);
  assert.equal(rulesFile(changes), "AGENTS.md");
  assert.deepEqual(changes.permissions.map((p) => p.agent), ["claude", "codex"]);
  assert.equal(lineOf(changes.actions[2]), "AGENTS.md (add block)");
  assert.equal(lineOf(changes.actions[0]), ".claude/skills/vnoc/SKILL.md (create)");
});

test("agent install: the same shape, not a dry run", () => {
  const changes = parseChanges(fixture("agent-install"));
  assert.equal(changes.dryRun, false);
  assert.deepEqual(changes.actions.map((a) => a.path), parseChanges(fixture("agent-install-dry-run")).actions.map((a) => a.path));
});

test("agent status: set up or not", () => {
  assert.equal(parseStatus(fixture("agent-status")).setUp, true);
  assert.equal(parseStatus('{"schema":1,"command":"agent status","set_up":false,"files":[],"missing":[]}').setUp, false);
});

test("a file that was left is told with its reason, and is not one of the files written", () => {
  const changes = parseChanges(
    JSON.stringify({ schema: 1, command: "agent install", dry_run: true, agents: ["claude"], actions: [{ part: "skill", path: ".claude/skills/vnoc/SKILL.md", change: "leave", agents: ["claude"], readers: [], why: "it was not written by V-NOC" }, { part: "rules", path: "AGENTS.md", change: "same", agents: ["claude"], readers: [], why: null }], permissions: [] }),
  );
  assert.equal(touching(changes).length, 0);
  assert.equal(left(changes).length, 1);
  assert.equal(lineOf(left(changes)[0]), ".claude/skills/vnoc/SKILL.md (leave: it was not written by V-NOC)");
});

test("an answer that is not JSON, of another command or of another schema is refused with a sentence", () => {
  assert.throws(() => parseList("error: nope"), /did not print JSON/);
  assert.throws(() => parseList(fixture("agent-status")), /another command/);
  assert.throws(() => parseStatus(fixture("agent-list")), /another command/);
  assert.throws(() => parseList('{"schema":2,"command":"agent list","agents":[]}'), /schema 2/);
  assert.throws(() => parseChanges('{"schema":1,"command":"agent install"}'), /no actions/);
});

test("the error line of a failure, without its word", () => {
  assert.equal(errorLine("error: unknown agent `vscode`; the names are claude, cline\n"), "unknown agent `vscode`; the names are claude, cline");
  assert.equal(errorLine("\n\nerror: second\n"), "second");
  assert.equal(errorLine("something else\n"), "something else");
  assert.equal(errorLine(""), undefined);
});

test("the offer: once, only where somebody uses an agent, and not when V-NOC's files are there", () => {
  assert.equal(decideOffer({ alreadyDecided: true, detected: ["claude"], setUp: false }), "decided");
  assert.equal(decideOffer({ alreadyDecided: false, detected: [], setUp: false }), "no-agent-here");
  assert.equal(decideOffer({ alreadyDecided: false, detected: ["other"], setUp: true }), "set-up");
  assert.equal(decideOffer({ alreadyDecided: false, detected: ["other"], setUp: false }), "offer");
});

test("the permissions: the dry run lists the settings file and Codex's rules file, and names the agents it can allow", () => {
  const changes = parseChanges(fixture("agent-install-permissions-dry-run"));
  assert.deepEqual(changes.canAllow, ["Claude Code", "Codex"]);
  assert.deepEqual(
    changes.actions.filter((a) => a.part === "permissions").map((a) => [a.path, a.change]),
    [
      [".claude/settings.local.json", "create"],
      ["~/.codex/rules/vnoc.rules", "create"],
    ],
  );
  // written, so there are no lines left to add by hand; and the confirmation shows them like any other file
  assert.deepEqual(changes.permissions, []);
  assert.equal(touching(changes).length, 6);
  assert.equal(lineOf(changes.actions[4]), ".claude/settings.local.json (create)");
  // without the flag the binary only prints the lines: nothing to allow was written, and the names are still given
  assert.deepEqual(parseChanges(fixture("agent-install-dry-run")).canAllow, ["Claude Code", "Codex"]);
  assert.deepEqual(parseChanges(JSON.stringify({ schema: 1, command: "agent install", actions: [] })).canAllow, []);
});

test("the question names the agents it covers, and the answer is one flag", () => {
  assert.equal(allowQuestion(["Claude Code"]), "Let Claude Code run V-NOC's read commands without asking");
  assert.equal(allowQuestion(["Claude Code", "Codex"]), "Let Claude Code and Codex run V-NOC's read commands without asking");
  assert.equal(allowFlag(true), "--permissions");
  assert.equal(allowFlag(false), "--no-permissions");
});

test("the command's argument skips the questions, and the read commands are allowed unless it says false", () => {
  assert.deepEqual(setUpArgument({ agents: ["claude"], confirm: true }), { agents: ["claude"], confirm: true, permissions: true });
  assert.deepEqual(setUpArgument({ agents: ["claude"], confirm: true, permissions: false }), { agents: ["claude"], confirm: true, permissions: false });
  assert.equal(setUpArgument({ agents: ["claude"], permissions: true })?.permissions, true);
  assert.equal(setUpArgument({ agents: ["claude"] })?.confirm, false);
  // anything else is the palette: the questions are asked
  assert.equal(setUpArgument(undefined), undefined);
  assert.equal(setUpArgument({ confirm: true }), undefined);
  assert.equal(setUpArgument({ agents: [1] }), undefined);
  assert.equal(setUpArgument("claude"), undefined);
});
