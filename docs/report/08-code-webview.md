# 08. Code: the webview canvas (`webview/`)

The canvas is a small **React** app built with **Vite**, drawn with **@xyflow/react** (React Flow, the graph library). It runs inside a VS Code **webview**: a sandboxed
page in an editor tab that can't read files or call the core. It only talks to the extension through `postMessage`.

## Files

| File | What it does |
|---|---|
| `src/main.tsx` | mounts `<App />` |
| `src/host.ts` | **the bridge**: `acquireVsCodeApi()` (allowed once per webview), `request(method)` posts `{type:"req", id, method}` and resolves when `{type:"res", id}` comes back |
| `src/App.tsx` | asks for `structure`, lays it out, draws it. Header with counts and timing, a Refresh button, light/dark from VS Code's body class, MiniMap and Controls |
| `src/layout.ts` | a simple deterministic tree layout (one row per visible node, depth to the right), **pure** so it's tested without a browser. Files start collapsed |
| `src/types.ts` | the structure types, **written by hand** to match `crates/vnoc/src/protocol.rs` (ts-rs or specta is an open question) |
| `src/layout.test.ts` | 6 tests (`npm test`, Node's built-in test runner) — ✅ all pass |
| `dist/` | the built bundle the extension loads |

## What you see today

A card per folder / file / class / function (icon + name, a red style for a syntax error), edges from parent to child. **Click** a folder, file or class to expand or collapse it.
The collapsed set is saved with `vscode.setState`, so it survives closing and reopening the tab. After drawing, the webview posts `{type:"rendered", visible, hidden}`, which the
extension test uses to prove the whole chain ran (bundle → host → core → driver → layout).

## Built vs left (all from `plan_vs/02-technology/03-frontend-webview.md` and 09-*)

| Built ✅ | Left ❌ (milestone) |
|---|---|
| React + xyflow canvas, simple tree layout, expand/collapse, saved state | **dagre** layout, left-to-right / top-to-bottom (M2) |
| one card type | the **node card** ported from old V-NOC (`EnhancedNode`, `NodeHeader`, `NodeCodeView`, `CodeViewDialog`…): `[▸]` expand, `[</>]` code inside the card, `[⤢]` zoom dialog, 🔗 ID, 📄 doc count (M2) |
| | **Canvas \| Code** tabs; the **focus stack** (focus, Back, Clear, breadcrumbs) (M2) |
| | **doc panel**: read, add, remove (M3); edit mode (M6) |
| | call-tree view with markers (recursion, cut, unresolved, external) (M5) |
| | **Monaco** editors as views of VS Code documents, ⌘S (M6, after spike **S5**) |
| | zustand store per tab, Tailwind mapped to `--vscode-*` theme variables (M2) |

Design docs: `plan_vs/09-webview/01…03` (all outlines; 09-01 and 09-02 can be written now, 09-03 waits for S5).

Next: [`09-plan-docs-guide.md`](09-plan-docs-guide.md)
