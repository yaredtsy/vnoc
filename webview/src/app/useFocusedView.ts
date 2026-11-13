import { useMemo } from "react";

import type { StructureResult } from "@vnoc/shared/messages.ts";
import { findNode } from "@vnoc/shared/tree.ts";
import { callRows } from "@/canvas/calls.ts";
import { type Layout, layout } from "@/canvas/layout.ts";
import { useCalls } from "@/state/calls.ts";
import { useSession } from "@/state/session.ts";
import { useView } from "@/state/view.ts";

/** What the canvas page draws, derived from the session, the call trees and the view: the layout, and the node the docs panel is about. */
export function useFocusedView(structure: StructureResult | null) {
  const focus = useSession((s) => s.contexts[s.active].focus);
  const selected = useSession((s) => s.contexts[s.active].selected);
  const openList = useSession((s) => s.contexts[s.active].expanded);
  const codeOpen = useView((v) => v.codeOpen);
  const trees = useCalls((s) => s.trees);

  const expanded = useMemo<ReadonlySet<string>>(() => new Set(openList), [openList]);
  // how many rows each card's "Calls" section has: the layout makes the card as tall as its rows
  const rowCounts = useMemo(() => new Map(Object.entries(trees).map(([key, tree]) => [key, callRows(tree.root).length])), [trees]);

  const root = structure?.root ?? null;
  const focusId = focus.length > 0 ? focus[focus.length - 1] : null;
  // zoomed in: the focused node is the root of what is drawn (and the root of a drawing is always open)
  const top = root && focusId ? findNode(root, focusId) ?? null : null;
  const laid = useMemo<Layout | null>(() => (root ? layout(top ?? root, expanded, codeOpen, rowCounts) : null), [root, expanded, top, codeOpen, rowCounts]);
  // the doc panel shows the selected card, else the focused node
  const selectedNode = root && selected ? findNode(root, selected) ?? null : null;

  return { root, top, laid, selected, docNode: selectedNode ?? top };
}
