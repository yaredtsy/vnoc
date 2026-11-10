import * as vscode from "vscode";

import type { TreeNode } from "@vnoc/shared/messages";
import { allNodes } from "@vnoc/shared/tree";
import type { SessionStore } from "./session";

// The search list (docs/10-ui-rework/02-sidebar.md §6): every node of the project by name or path. The native tree has no box of its own, so this is a
// QuickPick over the structure the extension already holds (no request to the core).

const ICON: Record<TreeNode["kind"], string> = { folder: "folder", file: "file", class: "symbol-class", function: "symbol-function" };

interface Item extends vscode.QuickPickItem {
  node: TreeNode;
}

/**
 * Moving through the list points at the node on the canvas (a highlight: nothing is selected, nothing opens). Picking one runs `open`, which selects it,
 * shows it where the user is working, and (through the session) selects its row in the tree. Closing the list clears the highlight.
 */
export function searchNodes(root: TreeNode, store: SessionStore, open: (node: TreeNode) => Promise<void>): void {
  const items: Item[] = allNodes(root)
    .filter((n) => n.id !== root.id) // the opened folder itself is not a node of the project
    .map((node) => ({ label: node.name, description: node.range ? `${node.path}:${node.range.start.line + 1}` : node.path, iconPath: new vscode.ThemeIcon(ICON[node.kind]), node }));

  const list = vscode.window.createQuickPick<Item>();
  list.placeholder = "Find a folder, file, class or function by name or path";
  list.matchOnDescription = true; // typing `billing/` finds by path
  list.items = items;
  list.onDidChangeActive((active) => store.dispatch({ type: "highlight", id: active[0]?.node.id ?? null }));
  list.onDidAccept(() => {
    const picked = list.selectedItems[0];
    list.hide();
    if (picked) void open(picked.node);
  });
  list.onDidHide(() => {
    store.dispatch({ type: "highlight", id: null });
    list.dispose();
  });
  list.show();
}
