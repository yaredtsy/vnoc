# 10.02: Testing

> **Status:** 📝 outline: written in full after **all spikes** · **Program:** all programs · **Milestones:** every milestone

**Purpose:** What is tested where, so each milestone's "done" (04-02) is checkable.

**Builds on:** [plan/07-02 testing strategy](../../plan/07-roadmap/02-testing-strategy.md) · [04-01 exit criteria](../04-roadmap/01-mvp-vs-stable.md)

## Will define

1. **Rust unit tests:** pure logic, table-driven (ladder, placement, the Missing / not-on-this-branch rule, ID derivation, `[[path]]` resolution).
2. **Store tests** against the `GraphStore` trait, run on both the SQLite and the in-memory store.
3. **Fixture repos:** the refactor suite from S4, the branch cases, a two-clone team setup (stable).
4. **Driver tests:** the protocol against a fake core; the stdout guard.
5. **Extension tests:** `@vscode/test-electron` for each milestone's user story.
6. **Webview tests:** components with Vitest; the editing race test from S5.
7. **Platform matrix:** CI OSes; manual runs in Remote SSH / WSL / dev containers before 1.0.

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| everything needed for the MVP exit criteria | the two-clone team test, 10k-file performance, remote environments |

## Questions to settle while writing

- Which public repos serve as fixtures (licence, size)?
