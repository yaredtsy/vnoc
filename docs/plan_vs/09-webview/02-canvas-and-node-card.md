# 09.02: The canvas, the node card and the focus stack

> **Status:** 📝 outline: written in full after **S1** · **Program:** webview (React) · **Milestones:** M2, M5

**Purpose:** The graph view: layout, the old V-NOC node card trimmed for VS Code, the focus stack, call trees.

**Builds on:** [02-03 node card + focus stack](../02-technology/03-frontend-webview.md)

## Will define

1. **Layout:** xyflow + dagre, left-to-right / top-to-bottom, re-layout on expand.
2. **The node card:** which old components are ported (`EnhancedNode`, `NodeHeader`, `NodeCodeView`, `CodeViewDialog`, `NodeDescription`, `NodeFooter`) and what is cut.
3. **Card controls:** expand, code toggle, zoom dialog, link icon (solid / outlined), doc count, status dot, dirty dot.
4. **Canvas | Code tabs.**
5. **The focus stack:** push, pushBulk, pop, clear, jump; the "node on top disappeared" rule.
6. **Call trees on the canvas:** frontier `+`, markers, dimmed pulled-in nodes, external leaves.

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| all of it | CodeLens-driven "open in canvas" entry points |

## Questions to settle while writing

- Performance limit: how many cards before we switch to compact nodes?
