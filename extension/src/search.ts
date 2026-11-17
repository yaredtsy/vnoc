import * as vscode from "vscode";

import type { SearchNode, SearchResult } from "@vnoc/shared/messages";
import { newest, placeOf } from "@vnoc/shared/search";
import type { Core } from "./core";
import type { SessionStore } from "./session";

// The search list (docs/20-search/05-on-screen.md §2): a QuickPick over `vnoc/search`. The core finds and ranks; this file asks and draws.

const ICON: Record<SearchNode["kind"], string> = { folder: "folder", file: "file", class: "symbol-class", function: "symbol-function" };

interface Item extends vscode.QuickPickItem {
  id: string;
}

/** A function inside a class has a method's icon, as in the sidebar. `alwaysShow`: VS Code must not filter or re-sort what the core ranked. */
function itemOf(row: SearchNode): Item {
  return {
    id: row.id,
    label: row.name,
    description: placeOf(row),
    iconPath: new vscode.ThemeIcon(row.kind === "function" && row.in_class ? "symbol-method" : ICON[row.kind]),
    alwaysShow: true,
  };
}

/**
 * Each change of the value asks the core at once (no debounce: a round trip is about 5 ms); only the newest answer sets the items (`newest`), and an empty value sets none. Moving through the list points at
 * the node on the canvas (a highlight: nothing is selected, nothing opens). Picking one runs `open`, which selects it, shows it where the user is working, and (through
 * the session) selects its row in the tree. Closing the list clears the highlight. `initial` is the value to start with, asked for at once.
 */
export function searchNodes(core: Core, store: SessionStore, open: (id: string) => Promise<void>, initial?: string): void {
  const list = vscode.window.createQuickPick<Item>();
  list.placeholder = "Type a name, a path or a few letters of a name (chor finds checkout_order; OrderService.par finds a method)";
  const numbering = newest();
  const ask = (text: string) => {
    if (text.trim() === "") {
      numbering.drop(); // an answer on its way is not wanted
      list.items = [];
      list.busy = false;
      return;
    }
    const number = numbering.next();
    list.busy = true;
    core.request<SearchResult>("vnoc/search", { query: text }).then(
      (result) => {
        if (!numbering.isNewest(number)) return;
        list.items = result.nodes.map(itemOf);
        list.busy = false;
      },
      () => {
        if (numbering.isNewest(number)) list.busy = false; // the core is not running: nothing to show, and the next change asks again
      },
    );
  };
  list.onDidChangeValue(ask);
  list.onDidChangeActive((active) => store.dispatch({ type: "highlight", id: active[0]?.id ?? null }));
  list.onDidAccept(() => {
    const picked = list.selectedItems[0];
    list.hide();
    if (picked) void open(picked.id);
  });
  list.onDidHide(() => {
    numbering.drop(); // an answer on its way is not wanted
    store.dispatch({ type: "highlight", id: null });
    list.dispose();
  });
  if (initial !== undefined && initial !== "") {
    list.value = initial;
    ask(initial);
  }
  list.show();
}
