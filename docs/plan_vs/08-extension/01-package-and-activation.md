# 08.01: `package.json` and activation

> **Status:** 📝 outline: written in full after **S1** · **Program:** extension host (TS) · **Milestones:** M0, M1

**Purpose:** What the extension declares to VS Code, and what happens from activation to a running core.

**Builds on:** [02-02 sidecar lifecycle](../02-technology/02-running-rust-sidecar.md) · [00-02 trust](../00-requirements/02-user-journey.md)

## Will define

1. **`contributes`:** commands, menus (explorer + editor context, per node kind), `viewsContainers` + `views`, `keybindings` (⌘S in the canvas), `configuration` (`vnoc.*`).
2. **Activation events:** `workspaceContains:.vnoc/vnoc.toml`, `workspaceContains:.vnoc/local.toml`, our commands and views.
3. **`capabilities.untrustedWorkspaces`:** limited, `restrictedConfigurations`.
4. **Starting the core:** the binary path inside the VSIX, `initialize`, restart, the Output channel.
5. **Folder layout** of `extension/` (src, test, the bundled `bin/` and `drivers/`).

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| all of it | contributions for IDs, playground, CodeLens |

## Questions to settle while writing

- Activation on any Python file, or only with `.vnoc/`?
