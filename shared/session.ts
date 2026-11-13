// The session: what more than one surface shows (focus, selection, the canvas's open nodes, the cursor node, a reveal). The extension host owns
// the only copy that changes; the sidebar reads it and a webview draws the snapshots it is sent (docs/10-ui-rework/01-state.md).
// Pure: no `vscode`, no React, tested with `node --test`.

import type { TreeNode } from "./messages";
import { type CallId, type CallLookup, callExists, isCallId, parseCallId } from "./calls.ts";
import { jump, pop } from "./focusStack.ts";
import { chainedPrefix, findNode, lineage, pathTo } from "./tree.ts";

export type ContextId = string;

/** One context: what the sidebar and the canvas show for it. MAIN, or an EXPLORE context opened from a call in another one (03 §4). */
export interface ContextState {
  id: ContextId;
  title: string;
  kind: "main" | "explore";
  /** the context it was opened from; null for MAIN */
  parent: ContextId | null;
  /** the call in `parent` this context explores; null for MAIN */
  call: CallId | null;
  /** the keys from the top level down to the focused node: a path, the last one is the root of what is drawn (R2) */
  focus: readonly string[];
  /** a structure key or a call id */
  selected: string | null;
  /** structure keys and call ids open on the canvas */
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
  | { type: "treeChanged" }
  /** a new call tree arrived for the function `owner`: what no longer resolves in it is dropped (05 §5) */
  | { type: "callsChanged"; owner: string }
  | { type: "explore"; call: CallId; activate: boolean }
  | { type: "activate"; id: ContextId }
  | { type: "closeContext"; id: ContextId };

export const MAIN: ContextId = "main";

/** The id of the EXPLORE context for a call: one per call, wherever it was opened from. */
export const exploreId = (call: CallId): ContextId => `explore:${call}`;

export function emptySession(): Session {
  const main: ContextState = { id: MAIN, title: "Main", kind: "main", parent: null, call: null, focus: [], selected: null, expanded: [] };
  return { contexts: { [MAIN]: main }, order: [MAIN], active: MAIN, cursor: null, highlight: null, reveal: null, seq: 0 };
}

const sameList = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

/** `old` itself when `next` has the same items, so a caller that compares by identity sees no change. */
const keep = (old: readonly string[], next: readonly string[]) => (sameList(old, next) ? old : next);

/**
 * Applies an intent. Returns THE SAME object when nothing changed, so the caller can skip the save and the broadcast, and keeps every array
 * that did not change, so a webview can tell from `===` what moved. `calls` says what a call id points at, when its tree is known.
 */
export function reduce(s: Session, context: ContextId, intent: Intent, tree: TreeNode, calls: CallLookup): Session {
  const c = s.contexts[context];
  if (!c) return s; // a late message from a context that is gone

  switch (intent.type) {
    case "select":
      return follow(change(s, context, { selected: intent.id }, { highlight: null }), context, intent.id, tree, calls, false);

    case "explore":
      // an unknown callee opens nothing: not even a selection
      return isCallId(intent.call) && calls(intent.call)?.key != null ? follow(change(s, context, { selected: intent.call }, { highlight: null }), context, intent.call, tree, calls, intent.activate) : s;

    case "focus": {
      // focusing a call is following it
      if (isCallId(intent.id)) return calls(intent.id)?.key != null ? follow(change(s, context, { selected: intent.id }, { highlight: null }), context, intent.id, tree, calls, true) : s;
      const way = lineage(tree, intent.id);
      if (!way || sameList(way, c.focus)) return s; // not in the tree, or already the last step: nothing
      return closeStale(change(s, context, { focus: way, selected: intent.id }, { highlight: null }), context);
    }

    case "focusBack":
      return closeStale(refocus(s, context, tree, pop(c.focus)), context);

    case "focusJump":
      return closeStale(refocus(s, context, tree, jump(c.focus, intent.depth)), context);

    case "toggle":
      return change(s, context, { expanded: c.expanded.includes(intent.id) ? c.expanded.filter((id) => id !== intent.id) : [...c.expanded, intent.id] });

    case "expand":
      return change(s, context, { expanded: keep(c.expanded, [...new Set([...c.expanded, ...intent.ids])]) });

    case "collapseAll":
      return change(s, context, { expanded: c.expanded.length === 0 ? c.expanded : [] });

    case "reveal": {
      const way = pathTo(tree, intent.id);
      if (!way) return s;
      // the focus is a path: steps are dropped from the end until the last one contains the node, never added
      let kept = c.focus.length;
      while (kept > 0 && !way.some((n) => n.id === c.focus[kept - 1])) kept--;
      const ancestors = way.slice(0, -1).map((n) => n.id);
      const next = closeStale(
        change(
          s,
          context,
          { focus: kept === c.focus.length ? c.focus : c.focus.slice(0, kept), expanded: keep(c.expanded, [...new Set([...c.expanded, ...ancestors])]), selected: intent.id },
          { highlight: null },
        ),
        context,
      );
      // `canvas: false` is the native sidebar's reveal (it scrolls its own row); it goes when the sidebar is a webview (step 8.5)
      if (!intent.canvas) return next;
      return { ...next, reveal: { context, id: intent.id, n: next.seq + 1 }, seq: next.seq + 1 };
    }

    case "revealed":
      return s.reveal?.n === intent.n ? { ...s, reveal: null, seq: s.seq + 1 } : s;

    case "cursor":
      return change(s, context, {}, { cursor: intent.id });

    case "highlight":
      return change(s, context, {}, { highlight: intent.id });

    case "activate":
      return s.contexts[intent.id] && s.active !== intent.id ? { ...s, active: intent.id, seq: s.seq + 1 } : s;

    case "closeContext": {
      const target = s.contexts[intent.id];
      return target && target.kind === "explore" ? removeContexts(s, [intent.id]) : s; // MAIN stays
    }

    case "treeChanged":
      return treeChanged(s, tree);

    case "callsChanged":
      return callsChanged(s, intent.owner, tree, calls);
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

// ── EXPLORE contexts (03 §4) ──────────────────────────────────────────────────────────────────────────────────────────

/**
 * After the selection of `context` was set: its EXPLORE children for any other call close (R4), and the one for the selected call opens when the
 * project knows the callee. `activate` also makes it the canvas's context. Not activated otherwise: the canvas stays where it was.
 */
function follow(s: Session, context: ContextId, id: string | null, tree: TreeNode, calls: CallLookup, activate: boolean): Session {
  let next = closeStale(s, context);
  if (id === null || !isCallId(id)) return next;
  const callee = calls(id);
  const way = callee?.key != null ? lineage(tree, callee.key) : null;
  if (!callee || callee.key === null || !way) return next; // a callee the project does not know: selected, nothing opens
  const child = exploreId(id);
  if (!next.contexts[child]) next = openExplore(next, context, { id: child, title: callee.name, kind: "explore", parent: context, call: id, focus: way, selected: callee.key, expanded: [] });
  return activate ? { ...next, active: child, seq: next.active === child ? next.seq : next.seq + 1 } : next;
}

/** Closes the EXPLORE children of `context` whose call is not what `context` has selected (with their own children). */
function closeStale(s: Session, context: ContextId): Session {
  const selected = s.contexts[context]?.selected ?? null;
  const stale = s.order.filter((id) => s.contexts[id].parent === context && s.contexts[id].kind === "explore" && s.contexts[id].call !== selected);
  return stale.length === 0 ? s : removeContexts(s, stale);
}

/** A new context goes right after the last of its parent's descendants (depth first: a child after its parent). */
function openExplore(s: Session, parent: ContextId, c: ContextState): Session {
  const subtree = new Set(descendants(s, [parent]));
  subtree.add(parent);
  const last = s.order.reduce((at, id, i) => (subtree.has(id) ? i : at), 0);
  const order = [...s.order.slice(0, last + 1), c.id, ...s.order.slice(last + 1)];
  return { ...s, contexts: { ...s.contexts, [c.id]: c }, order, seq: s.seq + 1 };
}

/** The roots' descendants (not the roots). `order` lists parents before children, so one pass finds them all. */
function descendants(s: Session, roots: readonly ContextId[]): ContextId[] {
  const set = new Set(roots);
  const out: ContextId[] = [];
  for (const id of s.order) {
    const parent = s.contexts[id].parent;
    if (parent !== null && set.has(parent) && !set.has(id)) {
      set.add(id);
      out.push(id);
    }
  }
  return out;
}

/** Closes the contexts and their children. If the active one is among them, the nearest ancestor that stays open becomes active. */
function removeContexts(s: Session, roots: readonly ContextId[]): Session {
  const gone = new Set([...roots, ...descendants(s, roots)]);
  let active = s.active;
  while (gone.has(active)) active = s.contexts[active].parent ?? MAIN;
  const contexts = Object.fromEntries(Object.entries(s.contexts).filter(([id]) => !gone.has(id)));
  return { ...s, contexts, order: s.order.filter((id) => !gone.has(id)), active, seq: s.seq + 1 };
}

// ── the code changed ──────────────────────────────────────────────────────────────────────────────────────────────────

/**
 * The focus after the code changed. A path that still holds is kept; one whose last steps are gone (deleted, renamed) is cut to the longest start
 * that exists. A 0.1 history (`[billing, Invoice.total]`: each step a place visited, not a child of the one before) becomes the lineage of its
 * last step that exists.
 */
function fixFocus(tree: TreeNode, focus: readonly string[]): readonly string[] {
  const start = chainedPrefix(tree, focus);
  if (start.length === focus.length) return focus;
  if (findNode(tree, focus[start.length]) === undefined) return keep(focus, start); // the step after the start is gone: a path that lost its end
  const last = [...focus].reverse().find((id) => findNode(tree, id) !== undefined)!;
  return lineage(tree, last) ?? start;
}

/**
 * A new call tree arrived for `owner` (05 §5). Every call id of that owner that is no longer in it is dropped from `expanded`; a `selected` that
 * was one falls back to the owner function; an EXPLORE context whose call is gone closes with its children. `calls` says `null` for gone and
 * `undefined` for "its tree is not held": that is unknown, not gone, and nothing is touched (a tree that failed to load closes nothing).
 */
function callsChanged(s: Session, owner: string, tree: TreeNode, calls: CallLookup): Session {
  const gone = (id: string | null): id is string => id !== null && isCallId(id) && parseCallId(id)?.owner === owner && calls(id) === null;
  const lost = s.order.filter((id) => gone(s.contexts[id].call));
  let next = lost.length === 0 ? s : removeContexts(s, lost);
  const home = findNode(tree, owner) ? owner : null;
  for (const id of next.order) {
    const c = next.contexts[id];
    next = change(next, id, { selected: gone(c.selected) ? home : c.selected, expanded: c.expanded.some(gone) ? c.expanded.filter((x) => !gone(x)) : c.expanded });
  }
  for (const id of [...next.order]) if (next.contexts[id]) next = closeStale(next, id);
  return next;
}

/** The code changed: every context drops what no longer exists, an EXPLORE context whose call is gone closes. */
function treeChanged(s: Session, tree: TreeNode): Session {
  const exists = (id: string) => (isCallId(id) ? callExists(tree, id) : findNode(tree, id) !== undefined);
  const lost = s.order.filter((id) => s.contexts[id].kind === "explore" && !(s.contexts[id].call !== null && callExists(tree, s.contexts[id].call!)));
  let next = lost.length === 0 ? s : removeContexts(s, lost);
  for (const id of next.order) {
    const c = next.contexts[id];
    next = change(next, id, {
      focus: fixFocus(tree, c.focus),
      selected: c.selected !== null && !exists(c.selected) ? null : c.selected,
      expanded: c.expanded.every(exists) ? c.expanded : c.expanded.filter(exists),
    });
  }
  for (const id of [...next.order]) if (next.contexts[id]) next = closeStale(next, id); // a selection that went takes its EXPLORE child with it
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

/**
 * One saved context, or null when it is not one. A context saved before EXPLORE existed has no `kind` and no `call`: it is MAIN (or, with a
 * parent, not valid, since nothing but MAIN existed).
 */
function readContext(v: unknown): ContextState | null {
  const c = v as Partial<ContextState> | null;
  if (typeof c !== "object" || c === null) return null;
  if (typeof c.id !== "string" || typeof c.title !== "string" || !isStrings(c.focus) || !isStrings(c.expanded)) return null;
  if (c.selected !== null && typeof c.selected !== "string") return null;
  if (c.parent !== null && typeof c.parent !== "string") return null;
  const kind = c.kind ?? (c.parent === null ? "main" : undefined);
  if (kind === "main") return c.parent === null ? { id: c.id, title: c.title, kind, parent: null, call: null, focus: c.focus, selected: c.selected, expanded: c.expanded } : null;
  if (kind === "explore") {
    return typeof c.call === "string" && parseCallId(c.call) !== null && c.parent !== null ? { id: c.id, title: c.title, kind, parent: c.parent, call: c.call, focus: c.focus, selected: c.selected, expanded: c.expanded } : null;
  }
  return null;
}

/**
 * The session to start with. `saved` is what `toSaved` wrote; `oldFocus` is the key the version before the session kept (a plain array), read once as
 * the focus of the main context. Anything that does not parse is dropped: no usable MAIN gives a fresh session; an EXPLORE context that is not valid
 * (a call that is not a call id, a parent that is not there or comes after it, a second one for the same call) is dropped with its children and MAIN stays.
 * Whether an EXPLORE context's call still exists is for the first `treeChanged`: the call trees are not saved, so they cannot be asked here.
 */
export function restoreSession(saved: unknown, oldFocus?: unknown): Session {
  const fresh = emptySession();
  const fallback = isStrings(oldFocus) ? { ...fresh, contexts: { [MAIN]: { ...fresh.contexts[MAIN], focus: oldFocus } } } : fresh;
  const s = saved as Partial<SavedSession> | null | undefined;
  if (typeof s !== "object" || s === null || typeof s.contexts !== "object" || s.contexts === null || !isStrings(s.order) || typeof s.active !== "string") return fallback;
  const read = (id: string) => readContext((s.contexts as Record<string, unknown>)[id]);
  if (read(MAIN)?.id !== MAIN) return fallback;

  const contexts: Record<ContextId, ContextState> = {};
  const order: ContextId[] = [];
  const calls = new Set<string>();
  for (const id of s.order) {
    const c = read(id);
    if (!c || c.id !== id || id in contexts) continue;
    if (c.kind === "main" ? id !== MAIN || order.length > 0 : !(c.parent! in contexts) || calls.has(c.call!)) continue; // MAIN first; a parent comes before its child
    contexts[id] = c;
    order.push(id);
    if (c.call) calls.add(c.call);
  }
  if (order[0] !== MAIN) return fallback;
  return { ...fresh, contexts, order, active: s.active in contexts ? s.active : MAIN };
}
