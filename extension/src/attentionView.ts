import * as path from "path";
import * as vscode from "vscode";

import { type NotReadRow, notReadRows } from "@vnoc/shared/attention.ts";
import type { MissingNode, Problem, StructureResult } from "@vnoc/shared/messages";

// NEEDS ATTENTION: what V-NOC found wrong, under STRUCTURE. Three groups: documented nodes the code no longer matches (Missing), files or IDs to look
// at (Problems), and what was not read (files the driver failed on, a structure cut at the file limit). It is there only while it has rows (`vnoc.hasAttention`). A Missing node is not in the tree, so nothing here touches the session.

type Which = "missing" | "problems" | "notRead";
const TITLES: Record<Which, string> = { missing: "Missing", problems: "Problems", notRead: "Not read" };

export type AttentionRow =
  | { row: "group"; which: Which; count: number }
  | { row: "missing"; node: MissingNode }
  | { row: "problem"; problem: Problem }
  | { row: "notRead"; item: NotReadRow };

const ICONS: Record<string, string> = { function: "symbol-function", class: "symbol-class", file: "file", folder: "folder" };

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

export class AttentionView implements vscode.TreeDataProvider<AttentionRow> {
  private readonly changed = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.changed.event;

  private missing: MissingNode[] = [];
  private problems: Problem[] = [];
  private notRead: NotReadRow[] = [];
  /** one row object per entry, so the tree keeps its open groups and its selection across a redraw of equal data */
  private readonly groups: Record<Which, AttentionRow> = { missing: { row: "group", which: "missing", count: 0 }, problems: { row: "group", which: "problems", count: 0 }, notRead: { row: "group", which: "notRead", count: 0 } };

  /** A new structure arrived. Draws again only when what this view shows is different. Returns whether it was. */
  update(structure: Pick<StructureResult, "missing" | "problems" | "failed" | "truncated" | "stats">): boolean {
    const notRead = notReadRows(structure);
    const same = JSON.stringify([structure.missing, structure.problems, notRead]) === JSON.stringify([this.missing, this.problems, this.notRead]);
    if (same) return false;
    this.missing = structure.missing;
    this.problems = structure.problems;
    this.notRead = notRead;
    this.groups.missing = { row: "group", which: "missing", count: this.missing.length };
    this.groups.problems = { row: "group", which: "problems", count: this.problems.length };
    this.groups.notRead = { row: "group", which: "notRead", count: this.notRead.length };
    this.changed.fire();
    return true;
  }

  get hasMissing(): boolean {
    return this.missing.length > 0;
  }

  get hasAttention(): boolean {
    return this.missing.length + this.problems.length + this.notRead.length > 0;
  }

  /** What the test suite looks at: the Missing IDs, the Problems, the labels of the Not read rows, and whether the view is shown. */
  snapshot(): { missing: string[]; problems: Problem[]; notRead: string[]; hasAttention: boolean } {
    return { missing: this.missing.map((m) => m.id), problems: this.problems, notRead: this.notRead.map((r) => r.label), hasAttention: this.hasAttention };
  }

  getChildren(row?: AttentionRow): AttentionRow[] {
    if (!row) return [this.missing.length > 0 ? this.groups.missing : [], this.problems.length > 0 ? this.groups.problems : [], this.notRead.length > 0 ? this.groups.notRead : []].flat();
    if (row.row !== "group") return [];
    if (row.which === "missing") return this.missing.map((node) => ({ row: "missing" as const, node }));
    if (row.which === "problems") return this.problems.map((problem) => ({ row: "problem" as const, problem }));
    return this.notRead.map((item) => ({ row: "notRead" as const, item }));
  }

  getTreeItem(row: AttentionRow): vscode.TreeItem {
    switch (row.row) {
      case "group": {
        const item = new vscode.TreeItem(`${TITLES[row.which]} (${row.count})`, vscode.TreeItemCollapsibleState.Expanded);
        item.id = `group:${row.which}`;
        item.contextValue = `${row.which}Group`;
        return item;
      }
      case "missing": {
        const m = row.node;
        // a def shows its name and where it was; a file or a folder shows its path and what it was
        const isDef = m.qualname !== "";
        const item = new vscode.TreeItem(isDef ? m.qualname : m.path, vscode.TreeItemCollapsibleState.None);
        item.id = `missing:${m.id}`;
        item.description = [isDef ? m.path : m.kind, plural(m.docs, "doc")].filter(Boolean).join(" · ");
        item.iconPath = new vscode.ThemeIcon((m.kind && ICONS[m.kind]) || (isDef ? "symbol-function" : "question"));
        item.tooltip = `${isDef ? `${m.path}::${m.qualname}` : m.path} is not in the code any more. Its docs are kept.`;
        item.contextValue = "missing";
        item.command = { command: "vnoc.openMissingDocs", title: "Open Docs", arguments: [row] };
        return item;
      }
      case "notRead": {
        const r = row.item;
        const item = new vscode.TreeItem(r.label, vscode.TreeItemCollapsibleState.None);
        item.iconPath = new vscode.ThemeIcon("warning");
        item.tooltip = r.tooltip;
        if (r.kind === "file") {
          item.id = `notRead:${r.path}`;
          item.description = r.reason;
          item.contextValue = "notRead";
          item.command = { command: "vnoc.openNotRead", title: "Open File", arguments: [row] };
        } else {
          item.id = "notRead:truncated";
          item.contextValue = "notReadTruncated";
        }
        return item;
      }
      case "problem": {
        const item = new vscode.TreeItem(row.problem.text, vscode.TreeItemCollapsibleState.None);
        item.iconPath = new vscode.ThemeIcon("warning");
        item.tooltip = row.problem.text;
        item.contextValue = "problem";
        if (row.problem.file) item.command = { command: "vnoc.openProblem", title: "Go There", arguments: [row] };
        return item;
      }
    }
  }
}

/** Where a problem's click goes. A folder is shown in the Explorer, never opened as a document. */
export async function goToProblem(problem: Problem): Promise<"revealed" | "opened" | "nowhere"> {
  if (!problem.file) return "nowhere";
  const uri = vscode.Uri.file(path.normalize(problem.file));
  let stat: vscode.FileStat;
  try {
    stat = await vscode.workspace.fs.stat(uri);
  } catch {
    throw new Error(`${problem.file} is not there any more.`);
  }
  if (stat.type & vscode.FileType.Directory) {
    await vscode.commands.executeCommand("revealInExplorer", uri);
    return "revealed";
  }
  const line = problem.line ?? 0;
  await vscode.window.showTextDocument(uri, { selection: new vscode.Range(line, 0, line, 0) });
  return "opened";
}
