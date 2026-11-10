# 05.04: S4 git and matching

**Goal:** docs survive refactors and branch switches in `none` mode (no in-code IDs): the MVP's hardest promise.
**Answers:** B6 (VS Code nudges, Rust reads git), C9 (`vscode.git` API behaviour + the pause rule, for speed only), C14 (the three gix lookups), VS-O10 (the similarity rule).
**Dropped (VS-D72, VS-D74):** C19 (the fresh-clone history walk), the per-branch-tip file check, the ancestry timing, the commit-graph comparison and the shallow-clone check.
**Time box:** 3 days. **Code:** `spikes/s4-git/`. **Needs:** S1; the S2 driver for fingerprints (`vnoc/defs`).

**Order (decided):** (1) the ladder first, because its results may change the default ID mode; (2) gix; (3) C9. Git sits behind a small `GitBackend` trait; the first implementation calls the `git` binary.

## Build

1. **The ladder + six refactor fixtures, plain `git` behind `GitBackend`.** A small Python repo with documented nodes, one branch per refactor:
   rename a function · move a file · rename a folder · move **and** edit a file · move a function to another file · rename a function **and** change its body
   (+ an extra: delete a function whose sibling looks the same). Run the ladder (02-05 §4) after each and record **which step** matched, or whether it was asked or Missing, and any wrong or missed match.
2. **VS-O10, the similarity rule.** Measure **two scores** per candidate: token 3-gram Jaccard and normalized token edit distance on the stripped body. Next to them record the **context features**:
   same file, same class, parameter-name overlap, name similarity. **A silent match needs all of:** score above a threshold, a **margin** over the second best, **same file or same class**, and a body of at least a
   **minimum token count**. Anything else asks the user. **Rule: zero wrong silent matches** (a wrong silent match is worse than a question). Tune on a **labelled set** built from real functions of open-source repos
   (`gen_labelled.py`: each function renamed and edited by 0-70%, placed in the same scope / another file / module level, plus a "deleted" variant), tune on some repos and **validate on others**.
3. **gix behind the same trait:** the three lookups VS-D74 needs: a path in HEAD's tree, the current branch, the local branch list. Anything slow or missing **falls back to the git binary through the same trait**; record which operations needed the fallback.
4. **C14, timings:** the three lookups on django, and on the shallow LightRAG copy (no history needed, so it should behave the same).
5. **The Missing / not-on-this-branch rule (VS-D74).** Test each case with the project **open and closed**: checkout to a branch without the function (docs committed on the other branch;
   docs committed on this one) · pull a teammate's deletion · rebase onto an upstream that deleted it · reset · a private-only node · a deleted branch · a detached HEAD (no notice).
6. **C9 and the pause rule.** What does `vscode.git` report during a rebase or merge in progress, and with `git.enabled = false`? Does our nudge still arrive? **Rule:** while a rebase, merge or cherry-pick is in progress
   (`.git/rebase-merge`, `.git/rebase-apply`, `MERGE_HEAD`, `CHERRY_PICK_HEAD`), **pause reconciling and run it once when the operation finishes.** For speed only: correctness doesn't depend on it (VS-D74).

> **🦀 Rust note: the three lookups with gix.**
> Nothing here walks history, so a shallow clone is no special case:
> ```rust
> let repo = gix::discover(root)?;                          // Err → no git repository → Missing
> let branch = repo.head_name()?;                           // None → detached HEAD
> let tree = repo.head_commit()?.tree()?;
> let committed = tree.lookup_entry_by_path("…/node.toml")?.is_some();
> let local: Vec<_> = repo.references()?.local_branches()?.collect();
> ```

## Measure

- The time of the three lookups (C14); which gix operations needed the git-binary fallback.
- A table: refactor × ladder step that matched (or Missing). This table becomes the MVP's refactor test suite (04-01 exit criterion 2).

## Pass / fail

- ✅ Every case in step 5 lands where VS-D74 says: docs on another branch don't prompt; the pulled deletion prompts; nothing prompts on a detached HEAD.
- ✅ Zero wrong silent matches on the fixtures; a threshold chosen for VS-O10.
- ✅ B6, C9, C14 have answers below; VS-O10 has a rule whose threshold was validated on repos it was not tuned on.
- ✅ The pause rule: a rebase, merge or cherry-pick in progress pauses reconciling, and it runs once at the end.

## Findings

*In progress (2026-11-06). Done: the ladder, the six fixtures (+ 2 extra), VS-O10 on the labelled set. Not done yet: the three gix lookups (C14), C9 and the pause rule.
Code: `spikes/s4-git/` (`spike-match`: `fixtures`, `tune`; `gen_labelled.py`). The S2 driver gained `vnoc/defs` (per-def fingerprints).*

**Ladder fixtures (git CLI behind `GitBackend`; nine documented nodes; both scores behave the same here):**
| Refactor | Result |
|---|---|
| rename a function | **step 4** (same body) |
| move a file | **step 2** (same file hash) |
| rename a folder | **step 2**, for functions, the file and the folder node (children's hashes) |
| move AND edit a file | **step 3** (git rename detection, `-M`) |
| move a function to another file | **step 4** |
| rename a function AND rewrite its body | **asked** when the ask-floor is 0 (the right function is the top candidate, jaccard 0.03, edit 0.32); **Missing** at the default floor 0.5 |
| delete a function whose sibling looks the same (extra) | **Missing**, not matched to the sibling (the sibling existed before, so it is no candidate) |
| two identical methods, both renamed in one commit (extra, VS-D73) | **both ask**, the likelier candidate first, **nothing silent** |
**No wrong silent match** in any fixture.

**VS-O10 on the labelled set** (real functions from LightRAG `HEAD`, httpx, requests, flask; 120 per repo, each renamed and edited 0-70%, placed in the same scope / module level / another file, plus a deleted variant;
~9,600 cases; tuned on LightRAG + httpx, validated on requests + flask; the silent rule needs score, margin, same file or class, and a minimum token count):
- Zero wrong silent matches is reachable, but only at a very high bar: **Edit score >= 0.99, Jaccard >= 0.91** (24 and 212 of 1,440 grid points). **Held-out: 0 wrong silent matches for both.**
- **There is a cliff, not a slope:** lowering the Edit threshold by 0.02 gives 186 wrong silent matches (train + held-out), by 0.05 gives 399.
- **So in practice only an edit-free rename or move is matched silently** (Edit, same scope: 317 of 480 at 0% edit, 20 of 476 at 5%, ~0 above 10%). **Anything edited is asked.**
- The right answer is usually offered first when asked: the true successor's mean Edit score is 0.91 at 5% edits and 0.62 at 35%, against 0.46 for the best wrong candidate.
- **Edit is the better score for asking**: with Jaccard, about 1,300 of 4,900 training cases fall below the ask floor and show as Missing; with Edit, none do.
- Names and parameters: name similarity of the true successor is flat at 0.64 (the generator renames every function), the parameter overlap falls with the edit level (1.00 to 0.04).
- **Consequence for the default ID mode:** without in-code IDs, a function that is renamed **and** edited in one commit becomes a "Find it…" question. Nothing is lost, and the answer is nearly always first in the list.
- **Twin cases** (the lost function has an identical twin in the corpus; 191 of 4,884 train cases, 388 of 4,711 held-out): under VS-D73 alone, **95 of 191 train twin cases and 51 of 388 held-out ones are silently wrong** (a new copy of the still-existing twin looks like the lost function's successor).
  Experiment: if a lost body with an identical twin anywhere **only asks** (never silent), twin wrong goes to **0** and silent-right drops from 446 to 444 (Edit) / 530 to 526 (Jaccard). **Adopted:** VS-D73's ask-only rules.

**Provisional tier (VS-D75).** *Measured on the synthetic labelled set (functions renamed and edited by a script). Not a decision input for v1: the tier is built and off (VS-D75). Re-measure on unobserved moves from real testers.*

**Code was restructured first, with no behaviour change** (the fixtures and 82 lines of tuning output are identical to the saved baseline), so the tiers are now one pure function (`ladder/policy.rs`, with tests) and the tuning harness calls the same `resolve` the ladder uses. See `spikes/s4-git/spike-match/README.md`.

**Measurement.** Cases that stay a *soft* question under the silent rule (the score or the lead is not good enough; not a twin, not a short body): 4,086 train, 3,729 held-out. For each, the top candidate by Edit score.

Top-1 accuracy per Edit-score band, **lead over the second >= 0.10** (train / held-out):
| band | train | held-out |
|---|---|---|
| 0.50-0.55 | 95% (94 cases) | 99% (154) |
| 0.55-0.60 | 98% (327) | 99% (424) |
| 0.60-0.65 | 100% (425) | 98% (484) |
| 0.65-0.70 | 93% (238) | 92% (202) |
| 0.75-0.80 | 91% (335) | 97% (301) |
| 0.80-0.85 | 71% (160) | 79% (205) |
| 0.85-0.90 | 77% (452) | 94% (371) |
| 0.90-0.95 | 70% (699) | 90% (527) |
| 0.95-1.00 | 68% (379) | 100% (277) |

**No floor reaches 95% top-1 accuracy.** Accuracy is **not monotone in the score**: a floor includes every band above it, and the high bands are the polluted ones. What a floor can reach (train / held-out):
| target | floor / lead | links | accuracy |
|---|---|---|---|
| 99%, 95% | none reaches it | | |
| 90% | 0.50 / 0.30 | 2,183 / 2,397 | 91.0% / 98.4% |
| 85% | 0.45 / 0.20 | 2,681 / 2,757 | 85.6% / 96.8% |
| 80% | 0.30 / 0.10 | 3,231 / 3,115 | 82.3% / 93.9% |

**Why the high bands are wrong** (Edit >= 0.80, lead >= 0.10): edits of 0-5%: **654 of 654 right** (train), 658 of 658 (held-out). Edits of 10-20%: 87% / 97% right. Edits of **35%+: 9 of 367 right** (0 of 86 held-out): the true successor only scores ~0.6, so a candidate at >= 0.80 is a lookalike. The simulation plants one on purpose (a copy of the closest sibling in the same file). In the 41 + 11 *deleted* cases any link is wrong.

**What would move from ask to provisional** (of 4,354 asked, train / of 4,134, held-out):
| floor / lead | moved | right | wrong (of which for a deleted function) |
|---|---|---|---|
| 0.50 / 0.30 | 2,183 / 2,397 | 1,987 / 2,359 | 196 (42) / 38 (14) |
| 0.45 / 0.20 | 2,681 / 2,757 | 2,296 / 2,670 | 385 (54) / 87 (27) |
| 0.30 / 0.10 | 3,231 / 3,115 | 2,659 / 2,924 | 572 (80) / 191 (44) |
Silent matches are unchanged (444 train, 423 held-out).

On the refactor fixtures, with floor 0.30 / lead 0.10 the one miss (rename a function **and** rewrite its body) becomes a **provisional, right** link; the twin methods still ask (`[Twin]`).

**Decided:** no floor in v1 (VS-D75). Neither the 90% floor (0.50 / lead 0.30) nor a near-twin rule is adopted for now; both stay as ideas for the real-data revisit.



Next: [`05-canvas-editors.md`](05-canvas-editors.md)
