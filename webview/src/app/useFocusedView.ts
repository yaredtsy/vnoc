import { useMemo } from "react";

import type { StructureResult } from "@vnoc/shared/messages.ts";
import { callFacts, callsOf } from "@vnoc/shared/calls.ts";
import { docSubject } from "@vnoc/shared/docSubject.ts";
import { findNode } from "@vnoc/shared/tree.ts";
import { CARD_W, CODE_H, type Layout, codeCardWidth, codeViewHeight, layout } from "@/canvas/layout.ts";
import { codeCharWidth } from "@/node-card/charWidth.ts";
import { useCalls } from "@/state/calls.ts";
import { openOf } from "@vnoc/shared/session.ts";
import { useSession } from "@/state/session.ts";
import { type CodeSize, useView } from "@/state/view.ts";

/** How wide a card with its code open wants to be: its columns (`state/view.ts`) at the code font's character width. */
function codeWidthOf(sizes: Readonly<Record<string, CodeSize>>): (id: string) => number {
  let charW: number | null = null; // measured when the first card with code asks
  return (id) => (sizes[id] === undefined ? CARD_W : codeCardWidth(sizes[id].columns, (charW ??= codeCharWidth())));
}

/** How tall its code view wants to be: its lines (`state/view.ts`) at the editor's line height; the most, until the code is read. */
const codeHeightOf = (sizes: Readonly<Record<string, CodeSize>>) => (id: string): number => (sizes[id] === undefined ? CODE_H : codeViewHeight(sizes[id].lines));

/** What the canvas page draws, derived from the session, the call trees and the view: the active context, the layout, and what the docs panel is about. */
export function useFocusedView(structure: StructureResult | null) {
  const active = useSession((s) => s.active);
  const contexts = useSession((s) => s.contexts);
  const focus = useSession((s) => s.contexts[s.active].focus);
  const selected = useSession((s) => s.contexts[s.active].selected);
  const openList = useSession((s) => openOf(s.contexts[s.active]));
  const codeOpen = useView((v) => v.codeOpen);
  const sitesOpen = useView((v) => v.sitesOpen);
  const sizes = useView((v) => v.codeSize);
  const trees = useCalls((s) => s.trees);
  const probes = useCalls((s) => s.probes);
  const states = useCalls((s) => s.states);

  const expanded = useMemo<ReadonlySet<string>>(() => new Set(openList), [openList]);

  const root = structure?.root ?? null;
  const focusId = focus.length > 0 ? focus[focus.length - 1] : null;
  // zoomed in: the focused node is the root of what is drawn (and the root of a drawing is always open)
  const top = root && focusId ? findNode(root, focusId) ?? null : null;
  // what a function, class or call card knows about its calls: its tree, what the host is doing, or a probe's count (05 §3.1)
  const callsFor = useMemo(() => (key: string) => callsOf({ trees, probes, states }, key), [trees, probes, states]);
  const laid = useMemo<Layout | null>(() => (root ? layout(top ?? root, expanded, codeOpen, callsFor, sitesOpen, root, codeWidthOf(sizes), codeHeightOf(sizes)) : null), [root, expanded, top, codeOpen, callsFor, sitesOpen, sizes]);
  // the doc panel follows the active context's selection (a selected call: its callee, and who calls it), else its focused node
  const subject = useMemo(() => (root ? docSubject({ contexts, active }, root, (call) => callFacts(trees, call)) : { node: null, calledBy: null }), [root, contexts, active, trees]);

  return { root, top, laid, selected, active, subject };
}
