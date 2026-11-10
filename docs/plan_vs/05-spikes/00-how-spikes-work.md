# 05.00: How spikes work

A **spike** is a small, throwaway program that answers questions we can't answer by thinking: "does VS Code really do X?", "how fast is Y?".
Each one also teaches one piece of VS Code extension work end to end, so together they're the **crash course** before real code.

## Rules

| Rule | Why |
|---|---|
| **Throwaway.** Code lives in `vnoc_playground/spikes/s<N>-<name>/` and is never copied into the product as is. | Spike code skips error handling and tests on purpose. Rewriting is faster than cleaning up. |
| **Time-boxed** (each file says how long). Out of time → write down what's known and stop. | A spike that runs for weeks is a hidden milestone. |
| **Answers named questions.** Each spike lists the checklist rows (`C…`, `B…`, `VS-O…`) it settles. | So we know when it's finished. |
| **Findings are written down** in the spike's own file (the *Findings* section), with numbers. | The design docs (`06-`…`10-`) are written from these. |
| **A finding that changes a decision** becomes a new row in the decision log. | Phase 1 stays the single source of truth. |

## The five spikes

| Spike | Teaches | Answers | Time box | Needs |
|---|---|---|---|---|
| [S1 hello core](01-hello-core.md) | extension basics, the Rust sidecar, webviews, trust | B5, C3, C6, C18 | 2 days | — |
| [S2 Python driver](02-python-driver.md) | the driver process, pylspt behind an adapter | C1, C2, C4, C11, C12, C17, C22 | 3 days | S1 |
| [S3 edits and saving](03-edits-and-saving.md) | `WorkspaceEdit`, save hooks, V-NOC trash | B8, C10, C16, C20 (+ confirms C8) | 2 days | S1 |
| [S4 git and matching](04-git-and-matching.md) | gix, HEAD awareness, the matching ladder | B6, C9, C14, VS-O10 | 3 days | S1 |
| [S5 canvas editors](05-canvas-editors.md) | Monaco in a webview, document views, ⌘S | C15, C21, C23 | 2 days | S1 |

```
S1 ──► S2
  ├──► S3      S3, S4, S5 only need S1's skeleton; do them in any order
  ├──► S4
  └──► S5
```

**Not covered by a spike, on purpose** (not needed for the MVP): C5 (which stores Cursor uses: checked during M0's CI work), C7 (MCP: phase 4),
C13 (HTTP proxy: only needed when downloads arrive in phase 4).

## Template for every spike file

```
Goal           one sentence
Answers        checklist rows
Time box       N days
Build          numbered steps, small enough to do one per sitting
Measure        what numbers to write down
Pass / fail    what "answered" means
Findings       (empty until the spike runs)
```

Next: [`01-hello-core.md`](01-hello-core.md)
