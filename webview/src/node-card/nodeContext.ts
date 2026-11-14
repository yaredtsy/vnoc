import { type Placed, canOpen } from "../canvas/layout.ts";

/** What the menu reads of a card or a sidebar row: a row has no layout, so it gives these and nothing else. */
export type NodeFacts = Pick<Placed, "id" | "key" | "role" | "kind" | "range" | "codeOpen" | "docs" | "link" | "childCount" | "callsChevron" | "collapsed"> & { call: { sites: readonly unknown[] } | null };

/**
 * What a card or a sidebar row tells VS Code about itself for its right-click menu. On a right-click VS Code collects `data-vscode-context` from the clicked
 * element and its ancestors, turns every key into a context key for the `when` clauses in package.json, and hands the object to the command.
 * One function, so a card and a row say the same things; `surface` says which one is asking (07 §6, 08 §6).
 */
export function nodeContext(f: { node: NodeFacts; contextId: string; isFocusRoot: boolean; surface?: "canvas" | "sidebar" }): string {
  return JSON.stringify({
    surface: f.surface ?? "canvas",
    // a call card whose callee the project does not know has no node to act on: its own section, so the node entries do not show (08 §6)
    webviewSection: f.node.role === "call" && f.node.key === null ? "callUnknown" : "node",
    preventDefaultContextMenuItems: true, // no Cut / Copy / Paste
    nodeKey: f.node.key ?? f.node.id, // a call card acts on its callee
    role: f.node.role,
    calleeKnown: f.node.role === "call" ? f.node.key !== null : undefined,
    sites: f.node.call?.sites.length,
    callId: f.node.role === "call" ? f.node.id : undefined,
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
