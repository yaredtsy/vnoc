import * as vscode from "vscode";

import type { CallLookup } from "@vnoc/shared/calls.ts";
import type { TreeNode } from "@vnoc/shared/messages";
import { type ContextId, type Intent, type Session, reduce, restoreSession, toSaved } from "@vnoc/shared/session.ts";

// The one copy of the session (docs/10-ui-rework/01-state.md). Everything that changes focus, selection, the canvas's open nodes, the cursor node
// or a reveal does it through `dispatch`; the sidebar reads `get()`, and every webview is sent the new snapshot from `onChange`.

const SAVED_KEY = "vnoc.session.v1";
/** What the version before the session kept: a plain array, the focus stack. Read once, then deleted. */
const OLD_FOCUS_KEY = "vnoc.focus";

export interface Change {
  prev: Session;
  next: Session;
}

export class SessionStore {
  private state: Session;
  private readonly emitter = new vscode.EventEmitter<Change>();
  /** Fires with the session before and after, so a listener can do only what moved. */
  readonly onChange = this.emitter.event;

  constructor(
    private readonly memento: vscode.Memento,
    /** the structure the intents are checked against; none before the first one arrives */
    private readonly tree: () => TreeNode | undefined,
    /** what a call id points at, from the call trees the host holds (EXPLORE contexts open from calls) */
    private readonly calls: CallLookup,
  ) {
    const old = memento.get<unknown>(OLD_FOCUS_KEY);
    this.state = restoreSession(memento.get<unknown>(SAVED_KEY), old);
    if (old !== undefined) {
      void memento.update(SAVED_KEY, toSaved(this.state)); // the converted focus must not be lost if nothing is dispatched
      void memento.update(OLD_FOCUS_KEY, undefined);
    }
    setContextKeys(undefined, this.state);
  }

  get(): Session {
    return this.state;
  }

  dispatch(intent: Intent, context: ContextId = this.state.active): void {
    const tree = this.tree();
    if (!tree) return; // nothing can be checked before the first structure arrives
    const prev = this.state;
    const next = reduce(prev, context, intent, tree, this.calls);
    if (next === prev) return;
    this.state = next;
    // the cursor, the highlight and a reveal are not saved: a cursor move must not write to disk
    if (next.contexts !== prev.contexts || next.order !== prev.order || next.active !== prev.active) void this.memento.update(SAVED_KEY, toSaved(next));
    setContextKeys(prev, next);
    this.emitter.fire({ prev, next });
  }
}

/** The context keys that menus and the view test: set only when the value moves. */
function setContextKeys(prev: Session | undefined, next: Session) {
  const keys = { "vnoc.focused": (s: Session) => s.contexts[s.active].focus.length > 0, "vnoc.hasSelection": (s: Session) => s.contexts[s.active].selected !== null };
  for (const [key, value] of Object.entries(keys)) {
    if (!prev || value(prev) !== value(next)) void vscode.commands.executeCommand("setContext", key, value(next));
  }
}
