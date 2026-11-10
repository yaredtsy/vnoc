import type * as vscode from "vscode";

import type { FocusStack } from "@vnoc/shared/focusStack";

const KEY = "vnoc.focus";

/**
 * The focus stack, owned here so the canvas and the sidebar share it: focusing a node narrows both to that node and what is under it.
 * Each UI asks for a change and shows what this holds. Saved per workspace, so it survives a restart.
 */
export class FocusState {
  private stack: FocusStack;

  constructor(private readonly memento: vscode.Memento) {
    this.stack = memento.get<string[]>(KEY, []);
  }

  get(): FocusStack {
    return this.stack;
  }

  /** Returns whether the stack changed (a UI that sends back what it was just given changes nothing, so nothing loops). */
  set(next: FocusStack): boolean {
    if (next.length === this.stack.length && next.every((id, i) => id === this.stack[i])) return false;
    this.stack = next;
    void this.memento.update(KEY, [...next]);
    return true;
  }
}
