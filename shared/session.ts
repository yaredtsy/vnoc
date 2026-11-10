// The session: what more than one surface shows (focus, selection, the canvas's open nodes, the cursor node, a reveal). The extension host owns
// the only copy that changes; the sidebar reads it and a webview draws the snapshots it is sent (docs/10-ui-rework/01-state.md).
// Pure: no `vscode`, no React, tested with `node --test`.

import type { TreeNode } from "./messages";
import { jump, pop, prune, push } from "./focusStack.ts";
import { findNode, pathTo } from "./tree.ts";

export type ContextId = string; // 0.1: always "main"

/** One context: what the sidebar and the canvas show for it. */
export interface ContextState {
  id: ContextId;
  title: string;
  /** the context it was opened from (0.2) */
  parent: ContextId | null;
  /** node keys; the last one is the root of what is drawn */
  focus: readonly string[];
  selected: string | null;
  /** node keys open ON THE CANVAS (the native tree keeps its own) */
  expanded: readonly string[];
}

export interface Session {
  contexts: Readonly<Record<ContextId, ContextState>>;
  /** top to bottom */
  order: readonly ContextId[];
  /** the one the canvas shows */
  active: ContextId;
  /** the node the text cursor is in; never saved */
  cursor: string | null;
  /** "look here" without selecting; never saved */
  highlight: string | null;
  /** a request for the canvas: "bring this into view"; never saved. `n` is unique, so a canvas can tell which one it did. */
  reveal: { context: ContextId; id: string; n: number } | null;
  /** grows on every change; a webview drops a session older than the one it shows */
  seq: number;
}

export type Intent =
  | { type: "select"; id: string | null }
  | { type: "focus"; id: string }
  | { type: "focusBack" }
  | { type: "focusJump"; depth: number }
  | { type: "toggle"; id: string }
  | { type: "expand"; ids: string[] }
  | { type: "collapseAll" }
  | { type: "reveal"; id: string; canvas: boolean }
  | { type: "revealed"; n: number }
  | { type: "cursor"; id: string | null }
  | { type: "highlight"; id: string | null }
  | { type: "treeChanged" };

export const MAIN: ContextId = "main";

export function emptySession(): Session {
  const main: ContextState = { id: MAIN, title: "Main", parent: null, focus: [], selected: null, expanded: [] };
  return { contexts: { [MAIN]: main }, order: [MAIN], active: MAIN, cursor: null, highlight: null, reveal: null, seq: 0 };
}

const sameList = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

/** `old` itself when `next` has the same items, so a caller that compares by identity sees no change. */
const keep = (old: readonly string[], next: readonly string[]) => (sameList(old, next) ? old : next);

/**
 * Applies an intent. Returns THE SAME object when nothing changed, so the caller can skip the save and the broadcast, and keeps every array
 * that did not change, so a webview can tell from `===` what moved.
 */
export function reduce(s: Session, context: ContextId, intent: Intent, tree: TreeNode): Session {
  const c = s.contexts[context];
  if (!c) return s; // a late message from a context that is gone

  switch (intent.type) {
    case "select":
      return change(s, context, { selected: intent.id }, { highlight: null });

    case "focus":
      return change(s, context, { focus: push(c.focus, intent.id), selected: intent.id }, { highlight: null });

    case "focusBack":
      return refocus(s, context, tree, pop(c.focus));

    case "focusJump":
      return refocus(s, context, tree, jump(c.focus, intent.depth));

    case "toggle":
      return change(s, context, { expanded: c.expanded.includes(intent.id) ? c.expanded.filter((id) => id !== intent.id) : [...c.expanded, intent.id] });

    case "expand":
      return change(s, context, { expanded: keep(c.expanded, [...new Set([...c.expanded, ...intent.ids])]) });

    case "collapseAll":
      return change(s, context, { expanded: c.expanded.length === 0 ? c.expanded : [] });

    case "reveal": {
      const way = pathTo(tree, intent.id);
      if (!way) return s;
      // the focus is a history, not a path: back to the newest entry whose node contains the one to show, else to nothing
      let kept = c.focus.length;
      while (kept > 0 && !way.some((n) => n.id === c.focus[kept - 1])) kept--;
      const ancestors = way.slice(0, -1).map((n) => n.id);
      const next = change(
        s,
        context,
        { focus: kept === c.focus.length ? c.focus : c.focus.slice(0, kept), expanded: keep(c.expanded, [...new Set([...c.expanded, ...ancestors])]), selected: intent.id },
        { highlight: null },
      );
      if (!intent.canvas) return next;
      return { ...next, reveal: { context, id: intent.id, n: next.seq + 1 }, seq: next.seq + 1 };
    }

    case "revealed":
      return s.reveal?.n === intent.n ? { ...s, reveal: null, seq: s.seq + 1 } : s;

    case "cursor":
      return change(s, context, {}, { cursor: intent.id });

    case "highlight":
      return change(s, context, {}, { highlight: intent.id });

    case "treeChanged":
      return treeChanged(s, tree);
  }
}

/** Replaces `focus` (Back, a crumb); keeps the selection if it is still under the new top, else selects the new top. */
function refocus(s: Session, context: ContextId, tree: TreeNode, focus: readonly string[]): Session {
  const c = s.contexts[context];
  if (focus === c.focus || sameList(focus, c.focus)) return s;
  const top = focus[focus.length - 1] ?? tree.id;
  const under = c.selected !== null && pathTo(tree, c.selected)?.some((n) => n.id === top);
  return change(s, context, { focus, selected: under ? c.selected : focus.length > 0 ? top : c.selected });
}

/** The code changed: every context drops what no longer exists. */
function treeChanged(s: Session, tree: TreeNode): Session {
  const exists = (id: string) => findNode(tree, id) !== undefined;
  let next = s;
  for (const id of s.order) {
    const c = next.contexts[id];
    next = change(next, id, {
      focus: prune(c.focus, exists),
      selected: c.selected !== null && !exists(c.selected) ? null : c.selected,
      expanded: c.expanded.every(exists) ? c.expanded : c.expanded.filter(exists),
    });
  }
  const gone = (id: string | null) => id !== null && !exists(id);
  return change(next, next.active, {}, { cursor: gone(next.cursor) ? null : next.cursor, highlight: gone(next.highlight) ? null : next.highlight, reveal: next.reveal && gone(next.reveal.id) ? null : next.reveal });
}

/** Applies a change to one context and to the top-level fields; the same object when no field is different (arrays are compared by identity). */
function change(s: Session, context: ContextId, patch: Partial<ContextState>, top: Partial<Pick<Session, "cursor" | "highlight" | "reveal">> = {}): Session {
  const c = s.contexts[context];
  const contextMoved = (Object.keys(patch) as (keyof ContextState)[]).some((k) => patch[k] !== c[k]);
  const topMoved = (Object.keys(top) as (keyof typeof top)[]).some((k) => top[k] !== s[k]);
  if (!contextMoved && !topMoved) return s;
  return { ...s, ...top, contexts: contextMoved ? { ...s.contexts, [context]: { ...c, ...patch } } : s.contexts, seq: s.seq + 1 };
}

/**
 * A session that arrived as a message has brand-new arrays even when only the cursor moved. A canvas lays out from `focus` and `expanded`, and a
 * new layout puts every dragged card back in its row: reuse the current arrays (and the reveal request) when their contents are equal.
 */
export function keepEqual(old: Session, next: Session): Session {
  const contexts = Object.fromEntries(
    Object.entries(next.contexts).map(([id, c]) => {
      const o = old.contexts[id];
      return [id, o ? { ...c, focus: keep(o.focus, c.focus), expanded: keep(o.expanded, c.expanded) } : c];
    }),
  );
  const reveal = old.reveal && next.reveal && old.reveal.n === next.reveal.n && old.reveal.id === next.reveal.id ? old.reveal : next.reveal;
  return { ...next, contexts, order: keep(old.order, next.order), reveal };
}

// ── saving ─────────────────────────────────────────────────────────────────────────────────────────────────────────────

/** The part of a session that survives closing the window. Not the cursor, the highlight or a reveal: a cursor move must not write to disk. */
export interface SavedSession {
  contexts: Record<ContextId, ContextState>;
  order: ContextId[];
  active: ContextId;
}

export function toSaved(s: Session): SavedSession {
  return { contexts: { ...s.contexts }, order: [...s.order], active: s.active };
}

const isStrings = (v: unknown): v is string[] => Array.isArray(v) && v.every((x) => typeof x === "string");

function isContext(v: unknown): v is ContextState {
  const c = v as ContextState | null;
  return typeof c === "object" && c !== null && typeof c.id === "string" && typeof c.title === "string" && (c.parent === null || typeof c.parent === "string") && isStrings(c.focus) && (c.selected === null || typeof c.selected === "string") && isStrings(c.expanded);
}

/**
 * The session to start with. `saved` is what `toSaved` wrote (anything that does not parse is dropped); `oldFocus` is the key the version before
 * the session kept (a plain array), read once as the focus of the main context.
 */
export function restoreSession(saved: unknown, oldFocus?: unknown): Session {
  const s = saved as SavedSession | null | undefined;
  const ok = typeof s === "object" && s !== null && typeof s.contexts === "object" && s.contexts !== null && Object.values(s.contexts).every(isContext) && isStrings(s.order) && typeof s.active === "string" && s.order.every((id) => id in s.contexts) && s.active in s.contexts;
  if (ok) return { ...emptySession(), contexts: s.contexts, order: s.order, active: s.active };
  const fresh = emptySession();
  if (!isStrings(oldFocus)) return fresh;
  return { ...fresh, contexts: { [MAIN]: { ...fresh.contexts[MAIN], focus: oldFocus } } };
}
