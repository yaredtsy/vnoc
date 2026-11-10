# 08.02: Commands, views and notifications

> **Status:** 📝 outline: written in full after **S1, S4** · **Program:** extension host (TS) · **Milestones:** M1–M4, M8, M9

**Purpose:** Every place the user meets V-NOC outside the canvas.

**Builds on:** [00-02 user journey](../00-requirements/02-user-journey.md) · [00-05 per-node controls](../00-requirements/05-user-controls-and-config.md)

## Will define

1. **Sidebar `TreeView`:** provider, lazy children, badges (📄 🔒 🆔 ⚠), the Missing / Not on this branch / Archived lists.
2. **Context menus** per node kind, enabled by `when` clauses (trusted, in scope, has docs…).
3. **Setup:** Quick look, the three setup QuickPicks, "Make it a project".
4. **Status bar:** indexing progress, the manual-sync count.
5. **Notifications:** the grouped Missing prompt, config errors, (stable) the Restore / Keep removed notice.
6. **Events forwarded to the core:** files, saves, renames, the `vscode.git` HEAD nudge.
7. **(Stable):** CodeLens, hover, search, effective config, playground in a terminal.

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| tree, menus, setup, status bar, Missing flow | CodeLens, hover, search, playground, effective config |

## Questions to settle while writing

- Tree sorting: by kind then name, or by source order?
