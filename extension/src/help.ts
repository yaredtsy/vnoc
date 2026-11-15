import * as os from "os";
import * as vscode from "vscode";

import { type Facts, helpEntries, type HelpEntry, helpTitle, type LogTee, reportUrl } from "./report.ts";

// Report Issue… and Help and Feedback… (03). Nothing is sent by V-NOC: the browser opens a GitHub form, and the user reads it before submitting.

export interface HelpDeps {
  ctx: vscode.ExtensionContext;
  log: LogTee;
  /** the core's version, or undefined when it is not running */
  coreVersion(): string | undefined;
}

const COPY_AND_OPEN = "Open Form and Copy Log";
const OPEN = "Open Form";

export function facts(d: HelpDeps): Facts {
  return {
    extension: String(d.ctx.extension.packageJSON.version),
    core: d.coreVersion(),
    editorVersion: vscode.version,
    editorName: vscode.env.appName,
    platform: process.platform,
    arch: process.arch,
    osType: os.type(),
    osRelease: os.release(),
  };
}

/** The flow of 03 §3.1: a modal that says what will happen, then the form; the log is on the clipboard only if asked. */
export async function reportIssue(d: HelpDeps): Promise<void> {
  const choice = await vscode.window.showInformationMessage(
    "Report an issue on GitHub",
    {
      modal: true,
      detail:
        "Your V-NOC, VS Code and system versions are filled in. The last 200 lines of the V-NOC log can be copied to your clipboard for you to paste into the form; " +
        "they can contain file and function names from your project. Nothing is sent from here: you read the form before you submit it.",
    },
    COPY_AND_OPEN,
    OPEN,
  );
  if (choice === undefined) return;
  if (choice === COPY_AND_OPEN) {
    await vscode.env.clipboard.writeText(d.log.tail());
    vscode.window.setStatusBarMessage("V-NOC log copied: paste it into the form's Log field.", 8000);
  }
  await vscode.env.openExternal(vscode.Uri.parse(reportUrl(facts(d))));
}

/** The QuickPick of 03 §4. */
export async function helpAndFeedback(d: HelpDeps): Promise<void> {
  const version = String(d.ctx.extension.packageJSON.version);
  const licence = String(d.ctx.extension.packageJSON.license);
  const pick = vscode.window.createQuickPick<vscode.QuickPickItem & { entry: HelpEntry }>();
  pick.title = helpTitle(version, d.coreVersion(), licence);
  pick.placeholder = "Report a problem, ask a question, or read more";
  pick.items = helpEntries(version, licence).map((entry) => ({ entry, label: entry.label, description: entry.description, detail: entry.detail }));
  const chosen = await new Promise<HelpEntry | undefined>((resolve) => {
    pick.onDidAccept(() => resolve(pick.selectedItems[0]?.entry));
    pick.onDidHide(() => resolve(undefined));
    pick.show();
  });
  pick.dispose();
  if (!chosen) return;
  if (chosen.url) return void (await vscode.env.openExternal(vscode.Uri.parse(chosen.url)));
  await vscode.commands.executeCommand(chosen.id === "report" ? "vnoc.reportIssue" : "vnoc.showLog");
}

/**
 * The core is not running and will not come back by itself (it could not start, or the client gave up restarting it). One notification per run; the
 * buttons are what a person does next. Restart is offered only when there is a client to restart.
 */
export async function coreStopped(reason: string, restart: (() => Promise<void>) | undefined): Promise<void> {
  const buttons = [...(restart ? ["Restart"] : []), "Show Log", "Report Issue"];
  const choice = await vscode.window.showErrorMessage(`V-NOC stopped: ${reason}`, ...buttons);
  if (choice === "Restart") await restart?.();
  else if (choice === "Show Log") await vscode.commands.executeCommand("vnoc.showLog");
  else if (choice === "Report Issue") await vscode.commands.executeCommand("vnoc.reportIssue");
}
