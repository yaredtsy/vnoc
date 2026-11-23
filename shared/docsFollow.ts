// What an unpinned docs tab shows (webview/src/docs/DocsApp.tsx): which of the cursor and the canvas selection moved last, and the node that gives. Pure: no `vscode`, no React.

import { type CallFacts, type CallId, isCallId } from "./calls.ts";
import type { TreeNode } from "./messages";
import { findNode } from "./tree.ts";

export type FollowSource = "cursor" | "selection";

/**
 * Which moved last. Only `selected` changed: the selection. Only `cursor` changed, or both at once (a tab's first snapshot brings both): the cursor wins when it is set, so a
 * tab opened from the editor follows the code. Both at once with no cursor: the selection. Neither changed: what it was.
 */
export function followSource(
  was: { cursor: string | null; selected: string | null; source: FollowSource },
  now: { cursor: string | null; selected: string | null },
): FollowSource {
  const cursorMoved = was.cursor !== now.cursor;
  const selectionMoved = was.selected !== now.selected;
  if (!cursorMoved && !selectionMoved) return was.source;
  if (selectionMoved && !(cursorMoved && now.cursor)) return "selection";
  return "cursor";
}

/** The source a tab starts with: the same rule as `followSource` for a first snapshot. */
export const firstSource = (now: { cursor: string | null; selected: string | null }): FollowSource => (now.selected && !now.cursor ? "selection" : "cursor");

/**
 * The node an unpinned tab shows: the pin first; else what moved last. A selected call card is its callee; a selection that gives no node (a call whose callee the project does not know or
 * whose tree this page does not hold, an id no longer in the tree) falls back to the cursor's node, never to nothing. `cursorNode` is the cursor's node already widened to what has docs.
 */
export function docsNode(input: {
  pinned: TreeNode | null;
  source: FollowSource;
  selected: string | null;
  cursorNode: TreeNode | null;
  root: TreeNode;
  facts: (call: CallId) => CallFacts | null;
}): TreeNode | null {
  const { pinned, source, selected, cursorNode, root, facts } = input;
  if (pinned) return pinned;
  if (source === "selection" && selected) {
    const key = isCallId(selected) ? facts(selected)?.callee.key : selected;
    const node = key != null ? findNode(root, key) : undefined;
    if (node) return node;
  }
  return cursorNode;
}
