# 04. Storage: the map

You asked for extra care here, so this domain is a folder. Read the files in order.

```
storage
├── README.md               this file: the four places, and how to choose between them
├── 01-vnoc-folder.md       B. the files users commit: layout and formats   ← the frozen part
├── 02-index.md             C. the in-memory index, and how SQLite replaces it later
├── 03-writing-safely.md    who is allowed to write what, and how a write survives a crash
└── 04-growth-path.md       basic → advanced: what each release adds, and why nothing ever migrates
```

## 1. The four places data lives

```
                      can V-NOC rebuild it?      who else sees it?        lives in
A. the user's code         it IS the truth          the whole team          the repo (git)
     + ID markers          no (a person chose it)   the whole team          the repo (git)
B. .vnoc/ files            no (a person wrote it)   the whole team          the repo (git)
C. the index               yes, from A + B          nobody                  memory  (0.1) → SQLite (later)
D. view state              not worth rebuilding     nobody                  VS Code's own storage
```

| Place | Holds | Examples |
|---|---|---|
| **A. Code + markers** | the program, and one small marker per documented node | `ID: 3RGJ…` in a docstring · `# FileID: …` · a `.vnoc-id` file in a folder |
| **B. `.vnoc/`** | everything a person created that cannot be computed again | docs (`.md`) · `node.toml` · `vnoc.toml` · the trash |
| **C. Index** | everything V-NOC can compute by reading A and B | the tree of folders/files/classes/functions · where each ID is · which nodes have docs · which are Missing |
| **D. View state** | how *you* were looking at it | collapsed nodes · the focus stack · scroll and zoom |

## 2. The one question that decides the place

Ask this about any new piece of data:

```
Did a person create it, and would they be upset if it vanished?
├── yes → is it about how THEY look at the project (zoom, open tabs)?
│         ├── yes → D. view state
│         └── no  → B. a file in .vnoc/       (or A, if it is the marker itself)
└── no  → C. the index
```

Two examples of getting this right:

- *"This node is Missing."* A person did not create that; V-NOC computed it by looking at the code. → **C**. If you stored it in `node.toml`, a branch switch
  would dirty committed files.
- *"This node is archived."* A person clicked Archive. → **B** (`status = "archived"` in `node.toml`).

## 3. Why the index can start in memory

Because of the table in §1: the index holds **nothing that cannot be rebuilt**. That is not an accident; it is the rule that makes it a cache.
So the only cost of having no database is time at start-up, and the only reason to add one later is to save that time. No user data ever moves.

That gives a clean order of work:

| Stage | Release | A. markers | B. `.vnoc/` | C. index | D. view state |
|---|---|---|---|---|---|
| **1. Basic** | 0.1 | docstring `ID:` · `# FileID:` · `.vnoc-id` | `vnoc.toml` · `nodes/` · `local/trash/` | in memory, rebuilt at start | webview `setState` |
| **2. Faster** | 0.2 | — | — | + call-tree results; SQLite if measured | — |
| **3. Richer** | 0.4 | + comment style | `[ids]` keys in `vnoc.toml` | + fingerprints for matching | — |
| **4. Private** | 0.5 | — | + `local/nodes/` · `local.toml` | — | — |
| **5. Advanced** | 0.6+ | — | + `body_hash` · playgrounds · assets | + doc links · search | — |

Every later stage is a `+`. Details: [`04-growth-path.md`](04-growth-path.md).

Next: [`01-vnoc-folder.md`](01-vnoc-folder.md)
