import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { atTag, DISCORD, ISSUE_FORM, REPO } from "./links.ts";
import { describeFacts, type Facts, helpEntries, helpTitle, LOG_LINES, LogTee, MAX_URL, reportUrl } from "./report.ts";

// The pure parts of Report Issue and Help and Feedback (docs/18-release-0.2/03-help-and-report.md §8). Run by `pnpm --filter vnoc unit`.

const facts: Facts = {
  extension: "0.2.0",
  core: "0.2.0",
  editorVersion: "1.146.0",
  editorName: "Visual Studio Code",
  platform: "darwin",
  arch: "arm64",
  osType: "Darwin",
  osRelease: "23.6.0",
};

// ── the form URL ────────────────────────────────────────────────────────────────────────────────────────────────────────

test("03 §3.2: the URL opens the bug form with the three facts, encoded", () => {
  const url = new URL(reportUrl(facts));
  assert.equal(`${url.origin}${url.pathname}`, `${REPO}/issues/new`);
  assert.equal(url.searchParams.get("template"), "bug_report.yml");
  assert.equal(url.searchParams.get("vnoc"), "0.2.0 (core 0.2.0)");
  assert.equal(url.searchParams.get("vscode"), "Visual Studio Code 1.146.0");
  assert.equal(url.searchParams.get("os"), "macOS arm64 (Darwin 23.6.0)");
  assert.ok(reportUrl(facts).startsWith(ISSUE_FORM));
});

test("03 §3.3: a core that is not running says so; an unknown platform is named as it is", () => {
  assert.equal(describeFacts({ ...facts, core: undefined }).vnoc, "0.2.0 (core not running)");
  assert.equal(describeFacts({ ...facts, platform: "freebsd", arch: "x64" }).os, "freebsd x64 (Darwin 23.6.0)");
  assert.equal(describeFacts({ ...facts, platform: "win32", arch: "x64", osType: "Windows_NT", osRelease: "10.0.22631" }).os, "Windows x64 (Windows_NT 10.0.22631)");
});

test("03 §3.2: values are encoded, and the URL is never longer than 8,000 characters, whatever a value holds", () => {
  const nasty = { ...facts, editorName: "a&b=c #d ".repeat(2000), osRelease: "é".repeat(5000) };
  const text = reportUrl(nasty);
  assert.ok(text.length <= MAX_URL, `${text.length} characters`);
  const url = new URL(text);
  assert.deepEqual([...url.searchParams.keys()].sort(), ["os", "template", "vnoc", "vscode"]);
  assert.ok(url.searchParams.get("vscode")!.startsWith("a&b=c #d"), "an & in a value stays inside the value");
});

test("03 §8: every query key of the URL is an `id` of the bug form (a renamed field fails here, not in a user's browser)", () => {
  const yaml = readFileSync(new URL("../../.github/ISSUE_TEMPLATE/bug_report.yml", import.meta.url), "utf8");
  const ids = new Set([...yaml.matchAll(/^\s+id:\s*(\w+)\s*$/gm)].map((m) => m[1]));
  for (const key of new URL(reportUrl(facts)).searchParams.keys()) {
    if (key !== "template") assert.ok(ids.has(key), `the form has no field \`${key}\` (it has ${[...ids].join(", ")})`);
  }
  assert.ok(ids.has("log"), "the form has the Log field the modal tells the user to paste into");
  assert.match(yaml, /^labels:\s*\[bug\]/m);
});

// ── the tee ─────────────────────────────────────────────────────────────────────────────────────────────────────────────

/** an output channel that remembers what it was given */
function fake() {
  const calls: string[] = [];
  return {
    calls,
    channel: {
      name: "V-NOC",
      append: (v: string) => void calls.push(`append:${v}`),
      appendLine: (v: string) => void calls.push(`appendLine:${v}`),
      replace: (v: string) => void calls.push(`replace:${v}`),
      clear: () => void calls.push("clear"),
      show: (a?: unknown, b?: unknown) => void calls.push(`show:${String(a)}:${String(b)}`),
      hide: () => void calls.push("hide"),
      dispose: () => void calls.push("dispose"),
    },
  };
}

test("03 §3.4: 250 lines in, the last 200 out, in order", () => {
  const { channel } = fake();
  const tee = new LogTee(channel);
  for (let i = 1; i <= 250; i++) tee.appendLine(`line ${i}`);
  const lines = tee.tail().split("\n");
  assert.equal(lines.length, LOG_LINES);
  assert.equal(lines[0], "line 51");
  assert.equal(lines.at(-1), "line 250");
});

test("03 §3.4: append without a newline joins the last line; a line still being written is in the tail", () => {
  const tee = new LogTee(fake().channel);
  tee.appendLine("one");
  tee.append("tw");
  assert.equal(tee.tail(), "one\ntw");
  tee.append("o\nthr");
  tee.append("ee\r\nfour");
  assert.equal(tee.tail(), "one\ntwo\nthree\nfour");
});

test("03 §3.4: the tee forwards every call to the real channel, and clear and replace reset what it keeps", () => {
  const { channel, calls } = fake();
  const tee = new LogTee(channel);
  tee.appendLine("a");
  tee.append("b");
  tee.show(true);
  tee.hide();
  assert.deepEqual(calls, ["appendLine:a", "append:b", "show:true:undefined", "hide"]);
  assert.equal(tee.name, "V-NOC");
  tee.replace("x\ny\n");
  assert.equal(tee.tail(), "x\ny");
  tee.clear();
  assert.equal(tee.tail(), "");
  tee.dispose();
  assert.equal(calls.at(-1), "dispose");
});

// ── Help and Feedback ───────────────────────────────────────────────────────────────────────────────────────────────────

test("03 §4: the list is the seven entries, in this order; the two that are commands have no URL, the rest open one", () => {
  const entries = helpEntries("0.2.0", "Apache-2.0");
  assert.deepEqual(entries.map((e) => e.id), ["report", "log", "discord", "star", "docs", "whatsNew", "licence"]);
  assert.deepEqual(entries.filter((e) => e.url === undefined).map((e) => e.id), ["report", "log"]);
  const url = (id: string) => entries.find((e) => e.id === id)!.url;
  assert.equal(url("discord"), DISCORD);
  assert.equal(url("star"), REPO);
  assert.equal(url("whatsNew"), atTag("0.2.0", "extension/CHANGELOG.md"));
  assert.equal(url("licence"), atTag("0.2.0", "LICENSE"));
  for (const e of entries) if (e.url) assert.match(e.url, /^https:\/\//);
});

test("03 §4: the title names the build; the licence entry names the licence and points to the third-party notices", () => {
  assert.equal(helpTitle("0.2.0", "0.2.0", "Apache-2.0"), "V-NOC 0.2.0 · core 0.2.0 · Apache-2.0");
  assert.equal(helpTitle("0.2.0", undefined, "Apache-2.0"), "V-NOC 0.2.0 · core not running · Apache-2.0");
  const licence = helpEntries("0.2.0", "Apache-2.0").find((e) => e.id === "licence")!;
  assert.match(licence.label, /Apache-2\.0/);
  assert.match(licence.detail ?? "", /THIRD_PARTY_NOTICES/);
});

test("every label of the list carries a codicon", () => {
  for (const e of helpEntries("0.2.0", "Apache-2.0")) assert.match(e.label, /^\$\([a-z-]+\) /, e.id);
});

// ── package.json ────────────────────────────────────────────────────────────────────────────────────────────────────────

const pkg = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));

test("03 §6: the Marketplace links are the ones links.ts opens, and the two commands are declared with the category V-NOC", () => {
  assert.equal(pkg.bugs.url, `${REPO}/issues`);
  assert.equal(pkg.homepage, `${REPO}#readme`);
  assert.equal(pkg.repository.url, REPO);
  assert.equal(pkg.qna, DISCORD);
  assert.ok(pkg.keywords.length <= 30, "the Marketplace allows 30 keywords");
  const declared = new Map<string, { title: string; category: string }>(pkg.contributes.commands.map((c: { command: string; title: string; category: string }) => [c.command, c]));
  assert.deepEqual([declared.get("vnoc.reportIssue")?.title, declared.get("vnoc.help")?.title], ["Report Issue…", "Help and Feedback…"]);
  for (const id of ["vnoc.reportIssue", "vnoc.help"]) assert.equal(declared.get(id)?.category, "V-NOC");
});

test("03 §5: Help and Feedback is one entry in the STRUCTURE view's ⋯ menu, in its own last group; Report Issue is in no menu", () => {
  const title = (pkg.contributes.menus["view/title"] as { command: string; when: string; group: string }[]).filter((e) => e.command === "vnoc.help");
  assert.deepEqual(title.map((e) => [e.when, e.group]), [["view == vnoc.structure", "9_help@1"]]);
  const everywhere = Object.entries(pkg.contributes.menus as Record<string, { command?: string }[]>).flatMap(([menu, entries]) => entries.filter((e) => e.command === "vnoc.reportIssue").map(() => menu));
  assert.deepEqual(everywhere, [], "Report Issue is in the palette and in the error states, not in a menu");
});
