# 03.01: Review checklist (the double-check before phase 2)

**Round 1** (2026-11-05): wiring, DB, frontend, call-tree default. Answered.
**Round 2** (2026-11-05): branch switches, Rust never writing source, Quick look storage, Workspace Trust, lazy-only, scoped index,
auto/manual sync, shared/local `.vnoc/`, team/solo. Recorded as **VS-D15 – VS-D26**.
**Round 3** (2026-11-05): monorepo, drivers as separate executables, core-managed downloads + six requirements, v1 bundling. Recorded as **VS-D27 – VS-D30**.
**Round 4** (2026-11-05): save reasons, third-party definitions + known limit, one anchor per node + shared-ID-wins merge, ancestry test, trash rule. Recorded as **VS-D31 – VS-D35**.
**Round 5** (2026-11-05): the storage + configuration doc (three places for data, ULID node folders, one `node.toml`, two-IDs merge, permanent IDs, `on-attach / all / none` +
`[[path]]` rules, the six-step ladder, archive expiry, the cache, config), plus your notes: **solo = local side only**, **ID removal by exact match only**, **the old node card + Canvas | Code tabs**.
Recorded as **VS-D36 – VS-D48**. My review of that doc is section D below.
**Round 6** (2026-11-05): your replies to section D. Remove ID by exact recorded span; **hash-derived IDs again**; every repo file is untrusted; never delete user writing
without a click; narrower env-change trigger; one `.vnoc/` per git root; 90/30-day retention; **editable node-card code**. Plus your note: **the driver owns ID placement** (Python: the docstring).
Recorded as **VS-D49 – VS-D57**.
**Round 7** (2026-11-05): ULID-style encoding (R13); sharing = moving folders, `.gitignore` never changes; one node can have data on both sides; file IDs as a top comment. Recorded as **VS-D58 – VS-D60**. Then: docs on the canvas (doc panel), code editable everywhere with ⌘S / Ctrl+S, solo `.gitignore` = `*`: **VS-D61 – VS-D63**.
**Round 8** (2026-11-05): save-hook facts (C8, C23), focus-stack keys via the core (B13), pylspt repo conditions (B14). Recorded as **VS-D64 – VS-D66**.
**Round 9** (2026-11-06): S4 findings and your notes: Missing by where `node.toml` lives, the provisional tier (built, off in v1), observed moves (step 0), doc freshness. Recorded as **VS-D74 – VS-D77**, plus an amendment to VS-D73.

What's left: answer each open row ✅ / ❌ / a change. Then the decision log moves 🟡 → ✅ and phase 2 starts.

---

## A. Product questions (only you can answer)

| # | Question | My proposal | Where |
|---|---|---|---|
| ~~A1~~ | ~~Default ID mode~~ → **decided, VS-D42**: `on-attach` (team) / `none` (solo) | ✅ | |
| A2 | Only nodes **with a permanent ID and user data** can be Missing / prompted; others vanish silently? | ✅ | VS-P2 |
| ~~A3~~ | ~~Remove and private docs~~ → **decided, VS-D35** | ✅ | |
| ~~A4~~ | ~~What Remove permanent ID does~~ → **decided, VS-D43**: exact-match line only; `id_in_code = "never"`; docs stay | ✅ | |
| A5 | Groups out of v1? | out | VS-P15 |
| A6 | Docs on call nodes in v1? | later | FR-53 |
| A7 | Playground in v1, or v1.1? | v1.1 if time is short | FR-60 |
| A8 | Tree shows **all files** in scope; non-Python files can have docs (`node.toml` only). | ✅ | VS-P21 |
| ~~A9~~ | ~~Docs in VS Code's own editor~~ → **decided, VS-D61**: a doc panel on the canvas (read, edit, add, remove) | ✅ | |
| A10 | Speed targets about right? | ✅ | NFR-01..05 |
| ~~A11~~ | ~~Setup questions~~ → **decided, VS-D47** | ✅ | |
| ~~A12~~ | ~~Pulled-in / third-party~~ → **decided, VS-D32** | ✅ | |
| ~~A13~~ | ~~`project.sharing` can't be overridden locally~~ → moot: sharing no longer changes `.gitignore` (VS-D58) | | |
| A14 | Driver cleanup after **30 days** unused. | ✅ | 02-07 §3 |
| ~~A15~~ | ~~N days~~ → **decided, VS-D56**: 90 / 30, configurable | ✅ | |
| ~~A16~~ | ~~Node card code read-only?~~ → **decided, VS-D57**: editable, as a view of VS Code's document | ✅ | |
| ~~A17~~ | ~~No module-level ID in v1?~~ → **decided, VS-D59**: a `# FileID:` comment at the top | ✅ | |

## B. Design proposals (you can just say "OK")

| # | Proposal | Where |
|---|---|---|
| ~~B1~~ | ~~`<slug>--<id>` folders~~ → **superseded by VS-D38** (ULID folders, sharded by the last 2 chars) | |
| ~~B2~~ | ~~Deterministic ID minting~~ → **superseded by VS-D41** (random ULIDs) | |
| ~~B3~~ | ~~Anchor fingerprint not rewritten on every edit~~ → moot: `node.toml` stores no fingerprint (VS-D39) | |
| ~~B4~~ | ~~Exactly-one-candidate rule~~ → **superseded by VS-D44** | |
| B5 | LSP framing + `vscode-languageclient` for the core | VS-P7 |
| B6 | File events come from VS Code; `vscode.git` only *nudges*; **Rust reads git itself** (gix) | VS-P9, VS-P28 |
| B8 | Rust writes directly only to the cache, `node.toml` files and `.gitignore`; everything a user can have open goes through `WorkspaceEdit` | VS-P24 |
| B11 | One writer per project (lock in `local/cache/`) | VS-P16 |
| B12 | Platform VSIX on Marketplace **and** Open VSX from the first release | VS-P13 |
| ~~B13~~ | ~~Focus stack rewrites keys from rename events~~ → **decided, VS-D64**: the core's `key_moves` table translates saved keys on restore; permanent IDs preferred | |
| ~~B14~~ | ~~pylspt stays its own repo~~ → **decided, VS-D65**: no dependency on `vnoc-driver-protocol`, adapter in `drivers/python`, pinned by `rev`, `[patch]` for dev | |
| B15 | The core embeds a *list* of minisign public keys (for key rotation) | 02-07 §3 |
| ~~B16~~ | ~~`anchor.toml` / `state.toml` split~~ → **superseded by VS-D39** (one `node.toml`) | |
| ~~B17~~ | ~~Node-card code read-only~~ → **superseded by VS-D57** (editable) | |
| ~~B18~~ | ~~Clear only resolution data on env change~~ → **decided, VS-D53** | |
| ~~B19~~ | ~~Expired private archives to the trash~~ → **decided, VS-D52** | |
| ~~B20~~ | ~~`# vnoc:` comment placement~~ → **superseded by VS-D54** (the driver owns placement; Python: docstring) | |
| ~~B21~~ | ~~Roots in one repo share one `.vnoc/`~~ → **decided, VS-D55** | |
| ~~B22~~ | ~~`.git/info/exclude` in solo~~ → **decided, VS-D63**: solo `.vnoc/.gitignore` = `*` | |

## C. Facts to verify in phase 2 (spikes, not opinions) **[check]**

| # | What | Why it matters |
|---|---|---|
| C1 | Does pylspt's `render_depth` bound the **solving**, or only the output? Time a depth-3 tree on a real repo. | Lazy is only lazy if solving is bounded. |
| C2 | Does pylspt / vendored ruff/ty ever write to **stdout** (inside the driver)? | stdout is the protocol. |
| C3 | `lsp-server` vs `async-lsp` / `tower-lsp-server` | Transport crate for both hops. **Answered by S1:** core = `async-lsp` (VS-D67); drivers = `lsp-server` (VS-D68). "Both hops" no longer holds. |
| C4 | Stripped/LTO binary size per platform | NFR-32. |
| C5 | Which stores Cursor / Windsurf install from | FR-90. |
| C6 | `#:schema` support in TOML extensions (Even Better TOML, Tombi) | FR-12, VS-D47. |
| C7 | VS Code's MCP registration API; opencode / pi config | VS-O1. |
| ~~C8~~ | ~~Save participant time budget~~ → **answered**: ~1.5 s shared by all listeners; edits dropped if the document changed; misbehaving listeners skipped → VS-D66 (precomputed edits) | FR-2B. |
| C9 | `vscode.git` API: HEAD change events, "rebase/merge in progress", `git.enabled = false`. The pause it feeds is **for speed only**: correctness doesn't depend on it (VS-D74). | VS-D15, VS-P28, VS-D74. |
| C10 | Can save-time IDs work in **manual sync** mode? | VS-D22. **Answered by S3 + VS-D71:** yes. Planning needs only the one buffer; the "taken?" check is the registry + a disk check of the ID folders, no source search. |
| C11 | macOS quarantine / Windows Defender on downloaded driver binaries | VS-D29. |
| C12 | Lock crate (`fd-lock` / `fs4`) on all OSes and network homes; `dirs::data_local_dir` on Windows | VS-D29.4–5. |
| C13 | HTTP client with proxy + rustls (`ureq` vs `reqwest`), and `http.noProxy` | VS-D29.6. |
| C14 | `gix`: the **three lookups** only: a path in HEAD's tree, the current branch, the local branch list. ~~`is_ancestor` speed, commit-graph, shallow behaviour, the ref check~~ dropped: VS-D74 has no ancestry test. | VS-D74. |
| C15 | Opening a library file **read-only** | VS-D32. |
| C16 | ~~Detecting "no system trash"~~ → **VS-D70: no system trash.** Now: moving files (and whole node folders) within `.vnoc/` on each OS, with open editors and Git watching them. Spike S3 checks macOS only. | VS-D70. |
| C17 | Per-function parse-error detection from the driver | VS-D31. |
| C18 | Watching `.vnoc/` when it sits **above** the opened folder (`RelativePattern` on the git root); does Workspace Trust cover it? | VS-D36. |
| ~~C19~~ | ~~`gix`: "last commit that changed path X" + reading the file at that commit~~ → **dropped, VS-D72**: a fresh clone uses `node.toml`'s anchors as written, then steps 1 and 6 only. | |
| C20 | pylspt: the new `# FileID:` writer (after shebang / encoding / leading comment block); `plan_ids` fed with given IDs; reading returns each `ID:`'s **exact span + text**; the remove edit inside a user-written docstring leaves it valid (and removes a V-NOC-only `"""ID: x"""` whole) | VS-D49, VS-D54. |
| C21 | Monaco inside a webview: CSP + web workers, bundle size, theme; the slice-edit version race (typing in the card and a regular editor at once) | VS-D57. |
| C22 | Which `pyproject.toml` sections count as "dependencies" across tools (`[project]`, `[dependency-groups]`, `[tool.poetry]`, `[tool.uv]`), and which lockfiles | VS-D53. |
| C23 | ✅ `document.save()` reports `Manual` (API docs). **Still to test:** the `cmd+s` / `ctrl+s` binding with `when: activeWebviewPanelId == 'vnoc.canvas'` really wins while the canvas has focus (VS Code then runs only our command), and the webview always reports the right document to save | VS-D62, VS-D66. |

## D. My review of the round-5 storage doc

Most of it is solid and fits the earlier rounds. These are the points worth a second look. Each is either fixed in the docs (✏️) or turned into a proposal above.

| # | Finding | What I did |
|---|---|---|
| R1 | **Random ULIDs bring back merge conflicts in source.** With hash-minted IDs, two branches that ID the same function wrote the *same* line, so git merged it cleanly. With random ULIDs they write two different `# vnoc:` lines at the same spot, which is a git conflict in the source file. That only happens when both sides put the ID *in code* (`all`, or `on-attach` with docs on both branches). | ✏️ Collision case 5 (02-04 §9). If the user keeps both lines → two IDs on one def → strong match → older ULID wins, the other line is removed by exact match. I'd accept the cost: random IDs are simpler and the case is rare. → **Round 6: VS-D50**, hash-derived IDs again. |
| R2 | **Your "no Remove ID unless we can detect it by ID" note** vs the doc's "Remove IDs in this folder". | ✏️ Both kept, under VS-D43: exact `# vnoc:<ULID>` line match only, never a pattern. The 26 random characters make an accidental match impossible. → **Round 6: VS-D49**, exact recorded span. |
| R3 | **Restricted Mode must cover `local.toml` too.** It's ignored only by *our* `.gitignore`; a malicious repo can commit its own `.vnoc/local.toml` naming an "interpreter". | ✏️ 00-05: executable fields ignored in both files while untrusted. → **Round 6: VS-D51**, every repo file is untrusted. |
| R4 | **"One `.vnoc/` per root" in multi-root** clashes with "at the git root" when two roots sit in the same repo. Also, when the opened folder is a sub-folder of the repo, `.vnoc/` is *outside* it (watching, trust). | ✏️ VS-P33 (they share one) + C18. → **Round 6: VS-D55**. |
| R5 | **Keeping `path`/`qualname` current in committed `node.toml` creates `git status` noise.** If a teammate renames a documented function without V-NOC running, every other teammate's sync writes the same change after pulling. The changes are identical, so they merge cleanly. | ✏️ Collision case 9, and "only rewritten when a sync confirms a move, never for not-on-this-branch". Accepting the noise is the price of fresh-clone matching. |
| R6 | *(Superseded by VS-D72: no history walk on a fresh clone.)* **Fresh-clone "before" state needs the file content at an old commit**, because `node.toml` stores no hashes. This fails on shallow clones (CI, `--depth 1`). | ✏️ 02-05 §4: shallow → only steps 1 and 6; C19. |
| R7 | **"Clear everything when `pyproject.toml` or the venv changes"** means every dependency bump re-indexes the whole repo. | VS-P30: clear only resolution data. → **Round 6: VS-D53**, with a narrower trigger. |
| R8 | **Where the ID comment goes isn't specified.** Decorators, shebang and encoding lines all matter. | VS-P32 + C20. → **Round 6: VS-D54**, the driver owns placement. |
| R9 | **Duplicate IDs on a fresh clone:** "the copy that matches the cached node" has no cache to go by. | ✏️ Use `node.toml`'s path + qualname; if neither matches, ask. |
| R10 | **`node:` frontmatter vs folder** when someone moves a `.md` by hand. | ✏️ The folder wins; frontmatter is fixed on the next save. |
| R11 | **Shared-vs-shared "older wins" may also need a source edit** (the loser's `# vnoc:` line). | ✏️ 02-04 §6: exact-match replace via `WorkspaceEdit`, trusted only. Every machine makes the same edit, so it merges cleanly. |
| R12 | **Auto-expiring private docs deletes user writing** that git doesn't have, and it broke NFR-10 ("user data is never deleted by an automatic process"). | VS-P31: expire to the trash; NFR-10 now names this one exception. → **Round 6: VS-D52**, and the fallback trash never auto-clears. |

### Round 6: two more from me

| # | Finding | What I did |
|---|---|---|
| R13 | **Base32 bit layout matters for sharding.** 16 bytes = 128 bits = 25.6 characters. Plain RFC 4648 base32 puts the 2 spare bits in the *last* character (only 8 possible values → 256 shards). Encoding like a ULID puts them in the *first* character, so the last two are full (1,024 shards). | ✏️ 02-05 §1. → **Round 7: VS-D60**, agreed. |
| R14 | **A docstring ID is part of `__doc__`**: it shows in `help()`, IDE hovers and Sphinx/pdoc output. The pasted D5 point ("a comment doesn't affect `__doc__`") doesn't hold for docstring IDs. The old V-NOC worked this way too. | ✏️ Written down as a trade-off in 02-05 §2. No change unless you want one. |

Next: [`02-phase-plan.md`](02-phase-plan.md) → phase 2 starts at [`../04-roadmap/01-mvp-vs-stable.md`](../04-roadmap/01-mvp-vs-stable.md)
