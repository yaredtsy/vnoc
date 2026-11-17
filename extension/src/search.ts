import * as vscode from "vscode";

import type { SearchNode, SearchResult } from "@vnoc/shared/messages";
import type { Core } from "./core";
import type { SessionStore } from "./session";

// The search list (docs/20-search/05-on-screen.md §2): a QuickPick over `vnoc/search`. The core finds and ranks; this file asks and draws.

/** How long the value must be still before the core is asked: the sidebar's box waits the same. */
const DEBOUNCE_MS = 50;

const ICON: Record<SearchNode["kind"], string> = { folder: "folder", file: "file", class: "symbol-class", function: "symbol-function" };

interface Item extends vscode.QuickPickItem {
  id: string;
}

/** A function inside a class has a method's icon, as in the sidebar. `alwaysShow`: VS Code must not filter or re-sort what the core ranked. */
function itemOf(row: SearchNode): Item {
  return {
    id: row.id,
    label: row.name,
    description: row.line === null ? row.path : `${row.path}:${row.line + 1}`,
    iconPath: new vscode.ThemeIcon(row.kind === "function" && row.in_class ? "symbol-method" : ICON[row.kind]),
    alwaysShow: true,
  };
}

/**
 * Each change of the value asks the core (after a pause); only the newest answer sets the items, and an empty value sets none. Moving through the list points at
 * the node on the canvas (a highlight: nothing is selected, nothing opens). Picking one runs `open`, which selects it, shows it where the user is working, and (through
 * the session) selects its row in the tree. Closing the list clears the highlight. `initial` is the value to start with, asked for at once.
 */
export function searchNodes(core: Core, store: SessionStore, open: (id: string) => Promise<void>, initial?: string): void {
  const list = vscode.window.createQuickPick<Item>();
  list.placeholder = "Type a name, a path or a few letters of a name (chor finds checkout_order; OrderService.par finds a method)";
  let newest = 0;
  let timer: NodeJS.Timeout | undefined;
  const ask = (text: string) => {
    const number = ++newest;
    if (text.trim() === "") {
      list.items = [];
      list.busy = false;
      return;
    }
    list.busy = true;
    core.request<SearchResult>("vnoc/search", { query: text }).then(
      (result) => {
        if (number !== newest) return;
        list.items = result.nodes.map(itemOf);
        list.busy = false;
      },
      () => {
        if (number === newest) list.busy = false; // the core is not running: nothing to show, and the next change asks again
      },
    );
  };
  list.onDidChangeValue((text) => {
    clearTimeout(timer);
    if (text.trim() === "") return ask(text); // an empty value clears the list at once
    timer = setTimeout(() => ask(text), DEBOUNCE_MS);
  });
  list.onDidChangeActive((active) => store.dispatch({ type: "highlight", id: active[0]?.id ?? null }));
  list.onDidAccept(() => {
    const picked = list.selectedItems[0];
    list.hide();
    if (picked) void open(picked.id);
  });
  list.onDidHide(() => {
    clearTimeout(timer);
    newest++; // an answer on its way is not wanted
    store.dispatch({ type: "highlight", id: null });
    list.dispose();
  });
  if (initial !== undefined && initial !== "") {
    list.value = initial;
    ask(initial);
  }
  list.show();
}
