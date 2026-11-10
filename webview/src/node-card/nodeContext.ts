import type { Placed } from "../canvas/layout.ts";

/**
 * What a card tells VS Code about itself for its right-click menu. On a right-click VS Code collects `data-vscode-context` from the clicked
 * element and its ancestors, turns every key into a context key for the `when` clauses in package.json, and hands the object to the command.
 * One function, so every card says the same things.
 */
export function nodeContext(f: { node: Placed; contextId: string; isFocusRoot: boolean }): string {
  return JSON.stringify({
    webviewSection: "node",
    preventDefaultContextMenuItems: true, // no Cut / Copy / Paste
    nodeKey: f.node.id,
    contextId: f.contextId,
    kind: f.node.kind,
    hasChildren: f.node.childCount > 0,
    open: f.node.childCount > 0 && !f.node.collapsed,
    isFocusRoot: f.isFocusRoot,
    hasCode: f.node.range !== null,
    codeOpen: f.node.codeOpen,
    docs: f.node.docs,
  });
}
