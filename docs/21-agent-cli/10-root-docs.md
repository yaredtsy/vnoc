# 21.10 Docs on the project root

Decided 2026-11-16 (C17): **planned, not urgent.** Built as step A17 if there is time; nothing else in this folder waits for it.

```
root docs
├── 1. what they are for
├── 2. where they live        one fixed folder, no ID
├── 3. naming the root
├── 4. what changes           the library · the commands · search
├── 5. in the extension       a second, optional commit
└── 6. what does not apply    Missing, relink, repair, markers
```

## 1. What they are for

A note about the whole project has no node today: a plan that touches three packages, the conventions of the code base, an overview an agent
should read first. `index/tree.rs` says "the root (the opened folder) has none", and `RepoPath` refuses an empty path, so `doc new` has nothing
to attach to.

## 2. Where they live

```
<git root>/.vnoc/
├── vnoc.toml
├── nodes/…
└── root/                      new: the project's own docs
    ├── overview.md
    └── plan-split-billing.md
```

**A fixed folder, no ID, no `node.toml`, no marker in the code.** An ID exists so a doc can follow code that moves or is renamed. The root does
neither relative to itself, and there is exactly one. So nothing has to be found again, and none of `vnoc-domain`'s ID rules are touched.

- "The root" is the **git root**, where `.vnoc/` is; not whichever subfolder an editor has open.
- A doc there has the usual frontmatter with `node: root` in place of an ID, and `hint: .`, and `author:` and `tags:` as any doc.
- **It is an addition** (`04-storage/01-vnoc-folder.md` §5 rule 1): a V-NOC that does not know `root/` reads only `nodes/` (`load_all` walks
  `nodes_dir`) and never sees it. No `format` change. The step adds `root/` to that file's layout.

## 3. Naming the root

`/` is the root: it is already the id of the tree's root node (`index/tree.rs`). A path that resolves to the root folder is the root too (`.`
when the current folder is the root).

```
vnoc doc new / --title "Plan: split billing in two" --tag plan
vnoc doc list /
vnoc doc read /
vnoc show /                the project's header, its docs, one level of tree
```

`node_ref` gains one form (`NodeRef::Root`) and `resolve_ref` one answer (`Found::Root`).

## 4. What changes

| Where | What |
|---|---|
| `userdata/layout.rs` | `root_dir` |
| `userdata/mod.rs` | `root_docs()`, `create_root_doc(title, body, …)`: the same slug, the same `create_new`, the same refusals as a node's doc (04 §4) |
| `userdata/read.rs`, `index` | the root's doc names are read with the records; the index holds them; a change to them moves the revision as any record's does |
| `userdata/doc_text.rs` | their text is kept and stamped like every doc's (doc 20, 03 §5) |
| `identity/attach.rs` | a target `Root`: no ID is chosen, no marker is planned, no `node.toml` is written; only the doc. The project must be initialized, as for any doc |
| `search.rs`, `vnoc-search` | a root doc is a doc whose node is the project: it passes `kind:folder`, and passes `in:` only when there is none |
| the commands | `doc new/list/read/path`, `show`, `status` (counted), `tree --docs` (listed above the first row), `find`, `grep --docs` |

## 5. In the extension

Not needed for agents, so it is its own commit after the commands work, and its own `[check]`s:

- the tree's root node carries its doc count when the opened folder **is** the git root (otherwise the root docs are reached by search, where
  a pick opens the file in a tab);
- **[check]** whether the pages treat `node_id` as an opaque text (then `"root"` can stand in it and the doc badge and the doc panel work as they
  are) or parse it; `vnoc/docs` and `vnoc/attach` accept `root` / `/`;
- **[check]** where a person would click: the top bar's project name, or the sidebar's title row. Decided by eye then.

## 6. What does not apply

A root doc is never Missing, is never relinked or repaired, and `check` has nothing to say about it. Removing one is deleting its file, as for
any doc.
