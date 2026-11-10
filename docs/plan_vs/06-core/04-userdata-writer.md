# 06.04: The `.vnoc/` writer (`vnoc-userdata`)

> **Status:** 📝 outline: written in full after **S3** · **Program:** Rust core · **Milestones:** M3

**Purpose:** Every rule about files a person created: where they go, how they move, and the guarantee that nothing user-written is deleted without a click.

**Builds on:** [02-04 §2–7](../02-technology/04-storage-and-git.md) · [00-05 config](../00-requirements/05-user-controls-and-config.md)

## Will define

1. **Layout and sharding:** `nodes/<last 2>/<id>/`, `local/nodes/…`, created only when something is attached.
2. **`node.toml`:** fields, the side rule (02-04 §5), read with `serde`, written with `toml_edit`.
3. **Docs:** frontmatter (`node:`, `hint:`), new-doc naming, the folder-is-the-list rule.
4. **Moves:** Share / Make private (file or whole node folder), merging into an existing folder, same-name clashes (`notes (2).md` + `vscode.diff`).
5. **`.gitignore` per mode** and the solo ↔ team switch.
6. **`FileOps` as the only output:** the use-case API (`attach_doc`, `share`, `archive`, `remove` …) returns operations; there is no public `write(path)`.
7. **The no-delete invariant as a type:** a `Delete` op exists only for "Empty V-NOC trash" and can only be built with a `UserConfirmed` token. A Remove is a move into `.vnoc/local/trash/`, not a delete (VS-D70).
8. **Config files:** `vnoc.toml` / `local.toml` reading, the last-valid-config rule, Restricted-Mode filtering of executable fields.

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| all of it, with `id_in_code` always `no` | `id_in_code = yes / never`, `[[path]]` rule updates on folder renames |

## Questions to settle while writing

- Doc file naming: from the title (`why-we-parse-twice.md`) or a short ID?
