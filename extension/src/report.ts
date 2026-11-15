import type * as vscode from "vscode";

import { changelogUrl, DISCORD, ISSUE_FORM, licenceUrl, REPO } from "./links.ts";

// What "Report Issue" and "Help and Feedback" decide, as plain functions (03). help.ts does the VS Code part: the message, the clipboard, the browser.
// This file is loaded by `node --test` as well, so it uses no syntax that needs a compiler (no parameter properties, no enums).

// ── the log tail ────────────────────────────────────────────────────────────────────────────────────────────────────────

/** The lines kept for a report: the output channel cannot be read back through the API, so the log is copied as it is written. */
export const LOG_LINES = 200;

/**
 * An output channel that forwards everything to a real one and keeps the last `LOG_LINES` lines. The language client writes the core's and the
 * driver's stderr through `appendLine` (it calls nothing else but `show` and `dispose`), so they land here too.
 */
export class LogTee implements vscode.OutputChannel {
  private lines: string[] = [];
  /** text appended without a newline yet: the next `append` continues it */
  private partial = "";
  readonly name: string;

  private readonly inner: vscode.OutputChannel;

  constructor(inner: vscode.OutputChannel) {
    this.inner = inner;
    this.name = inner.name;
  }

  append(value: string): void {
    this.inner.append(value);
    this.keep(value);
  }

  appendLine(value: string): void {
    this.inner.appendLine(value);
    this.keep(value + "\n");
  }

  replace(value: string): void {
    this.inner.replace(value);
    this.lines = [];
    this.partial = "";
    this.keep(value);
  }

  clear(): void {
    this.inner.clear();
    this.lines = [];
    this.partial = "";
  }

  // the two overloads of the API: `show(preserveFocus?)` and `show(column?, preserveFocus?)`
  show(columnOrPreserveFocus?: unknown, preserveFocus?: boolean): void {
    (this.inner.show as (a?: unknown, b?: boolean) => void)(columnOrPreserveFocus, preserveFocus);
  }

  hide(): void {
    this.inner.hide();
  }

  dispose(): void {
    this.inner.dispose();
  }

  /** The last `LOG_LINES` lines, oldest first; a line still being written is the last. */
  tail(): string {
    const all = this.partial === "" ? this.lines : [...this.lines, this.partial];
    return all.slice(-LOG_LINES).join("\n");
  }

  private keep(text: string): void {
    const parts = (this.partial + text).replace(/\r\n?/g, "\n").split("\n");
    this.partial = parts.pop() ?? "";
    this.lines.push(...parts);
    if (this.lines.length > LOG_LINES) this.lines.splice(0, this.lines.length - LOG_LINES);
  }
}

// ── the form ────────────────────────────────────────────────────────────────────────────────────────────────────────────

/** What the form's `vnoc`, `vscode` and `os` fields are filled with (03 §3.3). */
export interface Facts {
  /** the extension's version */
  extension: string;
  /** the core's version from `initialize`, or undefined when it is not running */
  core: string | undefined;
  /** `vscode.version` */
  editorVersion: string;
  /** `vscode.env.appName`: VS Code, Cursor, VSCodium… */
  editorName: string;
  platform: string;
  arch: string;
  /** `os.type()` and `os.release()` */
  osType: string;
  osRelease: string;
}

const PLATFORM_NAMES: Record<string, string> = { darwin: "macOS", win32: "Windows", linux: "Linux" };

export function describeFacts(f: Facts): { vnoc: string; vscode: string; os: string } {
  return {
    vnoc: `${f.extension} (core ${f.core ?? "not running"})`,
    vscode: `${f.editorName} ${f.editorVersion}`,
    os: `${PLATFORM_NAMES[f.platform] ?? f.platform} ${f.arch} (${f.osType} ${f.osRelease})`,
  };
}

/** GitHub refuses a URL much over 8 KB; the three values are short, and each is cut so the whole can never be long. */
export const MAX_URL = 8000;
const MAX_VALUE = 200;

/** The issue form, with its `vnoc`, `vscode` and `os` fields filled. The log is not in it (it goes on the clipboard); the user writes the title. */
export function reportUrl(facts: Facts): string {
  const values = describeFacts(facts);
  const query = Object.entries(values).map(([id, value]) => `${id}=${encodeURIComponent(value.slice(0, MAX_VALUE))}`);
  return `${ISSUE_FORM}&${query.join("&")}`;
}

// ── Help and Feedback ───────────────────────────────────────────────────────────────────────────────────────────────────

export type HelpId = "report" | "log" | "discord" | "star" | "docs" | "whatsNew" | "licence";

export interface HelpEntry {
  id: HelpId;
  /** the label, with its codicon */
  label: string;
  description?: string;
  detail?: string;
  /** opened in the browser; the two entries without one (`report`, `log`) run a command */
  url?: string;
}

/** The title of the list: which build the user is on, and under what licence. */
export function helpTitle(version: string, core: string | undefined, licence: string): string {
  return `V-NOC ${version} · core ${core ?? "not running"} · ${licence}`;
}

/** The list of 03 §4, in this order. The star is one line here and one in the README: never a pop-up (D3). */
export function helpEntries(version: string, licence: string): HelpEntry[] {
  return [
    { id: "report", label: "$(bug) Report Issue…" },
    { id: "log", label: "$(output) Show Log" },
    { id: "discord", label: "$(comment-discussion) Ask on Discord", url: DISCORD },
    { id: "star", label: "$(star) Star V-NOC on GitHub", description: "It helps other people find it.", url: REPO },
    { id: "docs", label: "$(book) Read the Docs", url: `${REPO}#readme` },
    { id: "whatsNew", label: `$(note) What's New in ${version}`, url: changelogUrl(version) },
    { id: "licence", label: `$(law) Licence: ${licence}`, detail: "Includes third-party code: see THIRD_PARTY_NOTICES", url: licenceUrl(version) },
  ];
}
