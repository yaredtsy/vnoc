import * as vscode from "vscode";

import type { TreeNode } from "@vnoc/shared/messages";

// The FOCUS view: the whole focus stack, one row per step, above STRUCTURE. A click jumps back to that step; the view is resizable, collapsible and
// movable like any view (VS Code does that for free). It reads the same session as everything else: nothing here keeps a stack of its own.

/** One row: step `depth` of the stack (1-based), or "All code" (depth 0). */
export interface Step {
  depth: number;
  /** the node key, none for "All code" */
  id: string | null;
  label: string;
  current: boolean;
}

export class FocusView implements vscode.TreeDataProvider<Step> {
  private readonly changed = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.changed.event;
  private steps: Step[] = [];

  /** Call with the stack and a way to name a node; draws again. */
  update(stack: readonly string[], find: (id: string) => TreeNode | undefined): void {
    this.steps = [
      { depth: 0, id: null, label: "All code", current: stack.length === 0 },
      ...stack.map((id, i) => ({ depth: i + 1, id, label: find(id)?.name ?? "?", current: i === stack.length - 1 })),
    ];
    this.changed.fire();
  }

  getChildren(): Step[] {
    return this.steps;
  }

  getTreeItem(step: Step): vscode.TreeItem {
    const item = new vscode.TreeItem(step.label, vscode.TreeItemCollapsibleState.None);
    item.id = `${step.depth}:${step.id ?? ""}`;
    item.description = step.current ? "●" : undefined;
    item.iconPath = new vscode.ThemeIcon(step.id ? "target" : "home");
    // "All code" has nothing to drop; every step has: this one and those after it
    item.contextValue = step.id ? "step" : "all";
    item.command = { command: "vnoc.focusJump", title: "Jump back to this step", arguments: [step.depth] };
    return item;
  }
}
