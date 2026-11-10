# 09.03: Editors in the canvas and the doc panel

> **Status:** 📝 outline: written in full after **S5** · **Program:** webview (React) · **Milestones:** M3 (doc panel: read, add, remove), M6 (editing)

**Purpose:** Monaco editors as views of VS Code documents, and the doc panel beside the graph.

**Builds on:** [02-03 docs on the canvas + editable everywhere](../02-technology/03-frontend-webview.md)

## Will define

1. **Monaco setup:** bundling, workers, CSP, theme, lazy loading.
2. **The slice editor component:** used by the card, the zoom dialog and the Code tab; version-stamped edits; dirty dot.
3. **⌘S:** reporting the focused document to the host.
4. **The doc panel:** toggle, doc list from both sides, Add (shared / private), Remove, rendered view (`react-markdown`), edit mode (Monaco Markdown), `vnoc://` links.

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| all of it | unchanged |

## Questions to settle while writing

- One Monaco instance reused, or one per open card?
