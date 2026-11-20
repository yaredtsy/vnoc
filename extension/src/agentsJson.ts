// *Set Up Coding Agents*: what the binary's `agent … --json` says, read into the few things the two questions need. **Pure.** The list of agents, which are
// detected, the files and what happens to each all come from the binary (P1): this file knows none of them, only the shape of the answers (`schema: 1`).

export interface AgentRow {
  name: string;
  detected: boolean;
  /** the skill's file, relative to the project (shown, never built here) */
  skill: string;
  /** the rules file this agent reads here */
  rules: string;
}

export interface FileAction {
  part: string;
  path: string;
  /** `create`, `replace`, `add_block`, `replace_block`, `same`, `leave` (the binary's words) */
  change: string;
  agents: string[];
  why: string | null;
}

export interface Changes {
  /** the project folder the binary worked in: the files' paths start there */
  root: string;
  dryRun: boolean;
  agents: string[];
  actions: FileAction[];
  /** the lines to add to an agent's own settings: the binary gives them for the agents whose syntax it knows, and only for those it did not write */
  permissions: { agent: string; text: string }[];
  /** the names (`Claude Code`) of the picked agents whose read commands V-NOC can allow: empty when none can, so there is nothing to ask */
  canAllow: string[];
}

export interface AgentStatus {
  setUp: boolean;
}

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;
const strings = (v: unknown): string[] => (Array.isArray(v) ? v.filter((s): s is string => typeof s === "string") : []);

/** The JSON value of a command's standard output; throws a plain sentence when it is not one, or when it is of another command or another schema. */
function value(text: string, command: string): Record<string, unknown> {
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch {
    throw new Error(`\`vnoc ${command} --json\` did not print JSON`);
  }
  if (!isObject(json) || json.command !== command) throw new Error(`\`vnoc ${command} --json\` printed the answer of another command`);
  if (json.schema !== 1) throw new Error(`\`vnoc ${command} --json\` has schema ${String(json.schema)}, which this V-NOC does not read: update the extension`);
  return json;
}

/** `agent list --json`. */
export function parseList(text: string): AgentRow[] {
  const json = value(text, "agent list");
  if (!Array.isArray(json.agents)) throw new Error("`vnoc agent list --json` has no agents");
  return json.agents.filter(isObject).map((a) => ({ name: String(a.name), detected: a.detected === true, skill: String(a.skill ?? ""), rules: String(a.rules ?? "") }));
}

/** `agent install --json`, with or without `--dry-run` (the same shape; an install that left a file prints it all the same, and exits 2). */
export function parseChanges(text: string): Changes {
  const json = value(text, "agent install");
  if (!Array.isArray(json.actions)) throw new Error("`vnoc agent install --json` has no actions");
  return {
    root: String(json.root ?? ""),
    dryRun: json.dry_run === true,
    agents: strings(json.agents),
    actions: json.actions.filter(isObject).map((a) => ({ part: String(a.part), path: String(a.path), change: String(a.change), agents: strings(a.agents), why: typeof a.why === "string" ? a.why : null })),
    permissions: Array.isArray(json.permissions) ? json.permissions.filter(isObject).map((p) => ({ agent: String(p.agent), text: String(p.text) })) : [],
    canAllow: strings(json.can_allow),
  };
}

/** `agent status --json`. */
export function parseStatus(text: string): AgentStatus {
  return { setUp: value(text, "agent status").set_up === true };
}

/** The names a user has ticked that are detected, for the list to start from. */
export const detectedNames = (rows: AgentRow[]): string[] => rows.filter((r) => r.detected).map((r) => r.name);

/** The actions that write or delete something (the binary says `same` and `leave` for the rest). */
export const touching = (changes: Changes): FileAction[] => changes.actions.filter((a) => a.change !== "same" && a.change !== "leave");

/** The actions that were refused: a file V-NOC did not write, or markers it cannot trust. `why` says. */
export const left = (changes: Changes): FileAction[] => changes.actions.filter((a) => a.change === "leave");

/** One action as a line of the confirmation: the path and what happens to it, in the binary's own word (`add_block` → `add block`). */
export const lineOf = (a: FileAction): string => `${a.path} (${a.change.replace(/_/g, " ")}${a.why ? `: ${a.why}` : ""})`;

/** The optional argument that skips the questions: `{ agents: string[], confirm: true, permissions?: boolean }` (`permissions` is true unless it says false). Anything else is the palette, and the questions are asked. */
export function setUpArgument(arg: unknown): { agents: string[]; confirm: boolean; permissions: boolean } | undefined {
  if (typeof arg !== "object" || arg === null) return undefined;
  const { agents, confirm, permissions } = arg as { agents?: unknown; confirm?: unknown; permissions?: unknown };
  if (!Array.isArray(agents) || !agents.every((a) => typeof a === "string")) return undefined;
  // the read commands are allowed unless the argument says not (the question's own default is ticked)
  return { agents: agents as string[], confirm: confirm === true, permissions: permissions !== false };
}

/** The question of the confirmation, ticked: "Let Claude Code and Codex run V-NOC's read commands without asking". */
export const allowQuestion = (names: string[]): string => `Let ${names.join(" and ")} run V-NOC's read commands without asking`;

/** The flag that carries the answer to it. */
export const allowFlag = (allow: boolean): string => (allow ? "--permissions" : "--no-permissions");

/** The rules file to offer to open after an install: the first action on a `rules` part. */
export const rulesFile = (changes: Changes): string | undefined => changes.actions.find((a) => a.part === "rules")?.path;

/**
 * The first line of what the binary said on standard error when it failed (`error: …`), without the word. `undefined` when it said nothing: the caller has
 * the exit code only.
 */
export function errorLine(stderr: string): string | undefined {
  const line = stderr.split(/\r?\n/).find((l) => l.trim() !== "");
  return line?.replace(/^error:\s*/, "").trim();
}

/** What the one-time offer asks (07 §3): after the first attach, when somebody here uses an agent and V-NOC's files are not there, and it was not decided before. */
export type OfferDecision = "offer" | "decided" | "no-agent-here" | "set-up";

export function decideOffer(facts: { alreadyDecided: boolean; detected: string[]; setUp: boolean }): OfferDecision {
  if (facts.alreadyDecided) return "decided";
  if (facts.detected.length === 0) return "no-agent-here";
  return facts.setUp ? "set-up" : "offer";
}
