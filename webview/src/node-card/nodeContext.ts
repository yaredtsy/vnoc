import type { ContextId } from "@vnoc/shared/session.ts";
import { type Placed, canOpen } from "../canvas/layout.ts";

/** What the menu reads of a card or a sidebar row: a row has no layout, so it gives these and nothing else. */
export type NodeFacts = Pick<Placed, "id" | "key" | "role" | "kind" | "range" | "codeOpen" | "docs" | "link" | "childCount" | "callsChevron" | "collapsed"> & { call: { sites: readonly unknown[] } | null };

/**
 * The extension of the file a node is in, with the dot (`.bend`), from its location key (`path::qualname`, or the path of a file); `""` for none. The menu
 * tests it against the extensions the core says have call trees (`fileExtname in vnoc.callTreeExtensions`): a webview has no `resourceExtname` of its own.
 */
export function fileExtname(key: string | null): string {
  const file = (key ?? "").split("::")[0];
  const name = file.slice(file.lastIndexOf("/") + 1);
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot) : "";
}

/**
 * What a card or a sidebar row tells VS Code about itself for its right-click menu (docs/16-ui-redesign/08-menus.md §6). On a right-click VS Code collects
 * `data-vscode-context` from the clicked element and its ancestors, turns every key into a context key for the `when` clauses in package.json, and hands the
 * object to the command. One function, so a card and a row say the same things; `surface` says which one is asking.
 *
 * A call card is a `node` section like any card: its entries test `role == call` (and `surface == canvas`), so a node's entries never show on it. `nodeKey` is
 * the structure node it acts on (a call card's callee; null for a callee the project does not know), `id` and `callId` say which card it is.
 */
export function nodeContext(f: { node: NodeFacts; contextId: ContextId; isFocusRoot: boolean; surface?: "canvas" | "sidebar" }): string {
  const call = f.node.role === "call";
  return JSON.stringify({
    surface: f.surface ?? "canvas",
    webviewSection: "node",
    preventDefaultContextMenuItems: true, // no Cut / Copy / Paste
    id: f.node.id,
    nodeKey: f.node.key,
    fileExtname: fileExtname(f.node.key),
    role: f.node.role,
    callId: call ? f.node.id : undefined,
    calleeKnown: call ? f.node.key !== null : true,
    sites: f.node.call?.sites.length ?? 0,
    contextId: f.contextId,
    kind: f.node.kind,
    hasChildren: canOpen(f.node),
    open: canOpen(f.node) && !f.node.collapsed,
    isFocusRoot: f.isFocusRoot,
    hasCode: f.node.range !== null,
    codeOpen: f.node.codeOpen,
    docs: f.node.docs,
    byName: f.node.link === "name",
  });
}

/** The empty part of a surface (08 §5): the canvas background, the sidebar's empty space. A card or a row inside it says its own section and wins. */
export const paneContext = (surface: "canvas" | "sidebar"): string => JSON.stringify({ surface, webviewSection: "pane", preventDefaultContextMenuItems: true });

/** A sidebar panel's header (08 §5): which context it is, and whether it is MAIN or an EXPLORE (only an EXPLORE has a Close). */
export const panelContext = (panel: { id: ContextId; kind: "main" | "explore" }): string =>
  JSON.stringify({ surface: "sidebar", webviewSection: "context", preventDefaultContextMenuItems: true, contextId: panel.id, contextKind: panel.kind });

/**
 * What a stop of a tour tells VS Code about itself for its right-click menu (docs/26-tours/design/04-route-and-map.md §7): a tour card on the map and a stop's row in the
 * route share one list, **Go to Code** and **Reveal in Main**. `nodeKey` is the structure node the stop is about (null for a stop whose node is gone).
 */
export const tourStopContext = (f: { tour: string; stop: string; nodeKey: string | null; surface: "canvas" | "sidebar" }): string =>
  JSON.stringify({ surface: f.surface, webviewSection: "tourStop", preventDefaultContextMenuItems: true, tour: f.tour, stop: f.stop, nodeKey: f.nodeKey });
