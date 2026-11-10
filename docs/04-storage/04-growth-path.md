# 04.04 The growth path: basic to advanced without a migration

```
growth
├── 1. the five stages
├── 2. why each step is only an addition
├── 3. the compatibility matrix (old V-NOC, new files)
├── 4. what WOULD force format = 2
└── 5. the test that keeps you honest
```

## 1. The five stages

```
Stage 1  BASIC      0.1    files only, index in memory
   │   + a saved copy of the index
   ▼
Stage 2  FASTER     0.2    SQLite cache under .vnoc/local/cache/ (only if measured)
   │   + a second way to write markers, more settings
   ▼
Stage 3  RICHER     0.4    comment-style IDs, [ids] in vnoc.toml, fingerprints in the cache
   │   + a second side with the same layout
   ▼
Stage 4  PRIVATE    0.5    .vnoc/local/nodes/, local.toml
   │   + more kinds of files in a node folder
   ▼
Stage 5  ADVANCED   0.6+   body_hash in frontmatter, playgrounds, assets, doc links, search
```

| Stage | New on disk | New in the index | Where the design already exists |
|---|---|---|---|
| 1 | `vnoc.toml` · `nodes/<shard>/<id>/{node.toml, *.md}` · `local/trash/` · markers | everything in [`02-index.md`](02-index.md) | this folder |
| 2 | `local/cache/index.sqlite` | call-tree expansions; the file stamps | `plan_vs/02-technology/04` §8 |
| 3 | `[ids] style = …`, `[[path]]` rules in `vnoc.toml`; `# vnoc:` comments in code | body fingerprints, "last known" state | `plan_vs/02-technology/05` §3–4 |
| 4 | `local/nodes/<shard>/<id>/*.md` · `local.toml` | a `side` on each doc | `plan_vs/02-technology/04` §3 |
| 5 | `body_hash:` in doc frontmatter · `play-*.py` · `assets/` | `doc_links`, a search table | `plan_vs/02-technology/04` §4 |

## 2. Why each step is only an addition

The layout chosen for stage 1 already has a place for each later thing:

| Later feature | Why stage 1 does not have to change |
|---|---|
| SQLite | It lives under `local/`, which `.vnoc/.gitignore` already ignores from day one. |
| Private docs | Same folder layout under `local/nodes/`. Same ID, same shard. The loader reads two roots instead of one. |
| Comment-style IDs | A new *marker syntax*, read by the driver. The ID text, `node.toml` and the node folder do not change. Drivers read **both** styles from then on, so switching style never orphans an old ID. |
| No-marker mode | Already valid in stage 1: `id_in_code = "no"`. |
| Archive | The fields are already in `node.toml`. Only the UI is missing. |
| Doc freshness | One more frontmatter line. Old docs without it simply show no freshness hint. |
| Playgrounds, assets | More files in the node folder. 0.1's doc list only looks at `*.md`, so it does not see them. |
| More languages | `qualname` is "the language's own name" and opaque to the core. `kind` accepts unknown values. |

## 3. The compatibility matrix

The case that matters on a team: **Alice has a newer V-NOC than Bob**, and both commit to the same repo.

| Alice's newer V-NOC writes… | Bob's 0.1 does… | OK? |
|---|---|---|
| a new key in `node.toml` | keeps it when it rewrites the file (`toml_edit`) | ✅ |
| a new value in a known key (`id_in_code = "sometimes"`) | reads it as `Other`, leaves it alone | ✅ (already implemented) |
| a new frontmatter line | ignores it; does not rewrite frontmatter unless the user saves the doc, and then rewrites only its own lines | ✅ if you follow rule 1 of [`01-vnoc-folder.md`](01-vnoc-folder.md) §5 |
| a new file type in a node folder | does not list it, does not touch it; Remove moves the whole folder, so it travels to the trash and back | ✅ |
| a comment-style ID marker | does not see the marker → finds the node by its anchor instead → shows it as "attached by name" | ✅ degraded, not broken |
| `format = 2` | opens read-only, says "this project needs a newer V-NOC" | ✅ |

The row to protect is the first one. If any code path ever rebuilds a `node.toml` from a struct (instead of editing the document), Bob silently deletes
Alice's fields. You have a test for byte-for-byte round trips today; keep it forever.

## 4. What would force `format = 2`

Only a change that makes an older V-NOC do something **harmful**, not merely something incomplete. For example:

- changing how the shard is computed (old versions would look in the wrong folder and create duplicates)
- changing the ID text format (length, alphabet)
- changing the *meaning* of an existing key

Everything in §1 is far from this list. With care you may never need `format = 2`. That is the goal.

## 5. The test that keeps you honest

From 0.2 on, keep one small sample project per released version:

```
crates/vnoc/tests/fixtures/
├── project-0.1/        a tiny repo: 3 Python files, .vnoc/ with 4 nodes, written by the real 0.1
├── project-0.2/
└── …
```

One test per fixture, run by every later version:

```rust
#[test]
fn opens_a_0_1_project_unchanged() {
    let dir = copy_fixture("project-0.1");
    let index = build_index(&dir);
    assert_eq!(index.missing(), vec![]);                    // every doc is still found
    assert_eq!(count_docs(&index), 5);
    assert!(git_status_is_clean(&dir));                     // opening a project must not change a single byte
}
```

The last line is the important one. "Opening an old project dirties `git status`" is the most common way tools like this annoy a team, and this test makes
it impossible to ship by accident.

Next: [`../05-identity/`](../05-identity/README.md)
