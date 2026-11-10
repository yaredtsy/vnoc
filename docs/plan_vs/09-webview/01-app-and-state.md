# 09.01: The webview app and its state

> **Status:** 📝 outline: written in full after **S1** · **Program:** webview (React) · **Milestones:** M2

**Purpose:** How the React app is structured, how it talks to the host, and how it survives being hidden or restarted.

**Builds on:** [02-03 webview stack](../02-technology/03-frontend-webview.md)

## Will define

1. **App structure:** folders for `bridge/`, `store/`, `canvas/`, `card/`, `editors/`, `docs/`.
2. **The bridge:** typed messages, the host's allow-list, request IDs.
3. **State:** one zustand store per tab; what is saved with `vscode.setState`; the serializer.
4. **Restoring a tab:** translating saved keys through `keys.translate` (VS-D64) before drawing.
5. **Theme:** Tailwind mapped to `--vscode-*` variables, high contrast.
6. **Restricted Mode UI:** what's hidden or read-only.

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| all of it | unchanged |

## Questions to settle while writing

- Port shadcn pieces, or plain components on VS Code variables?
