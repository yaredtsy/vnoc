# 18.03 Help and report: one way to tell us, from inside VS Code

```
help and report
├── 1. what a user needs, and what is there today
├── 2. the two commands
├── 3. Report Issue: the flow, the form, what is sent
├── 4. Help and Feedback: the list (Discord, Star, Licence, …)
├── 5. where they appear
├── 6. package.json: bugs, homepage, qna
├── 7. what is not in this (telemetry, Sentry)
├── 8. tests
└── 9. the docs this step updates
```

Step R1 of the README. A new job for 16/01 §2: **J9, "tell us what broke, or find help"**. Every control below serves it.

## 1. What a user needs, and what is there today

| Need | Today |
|---|---|
| "it broke: tell the author, with enough to fix it" | nothing in VS Code; VS Code's own *Help: Report Issue* lists V-NOC but knows no URL (no `bugs` in `package.json`) |
| "what went wrong?" | *V-NOC: Show Log* in the palette; the sidebar's error state has Try Again and Show Log (16/06 §8) |
| "where do I ask a question / meet others?" | nowhere: the Discord is only in the old project's README (`https://discord.gg/J5nfPHqyBr`) |
| "is this project alive, can I support it?" | the Marketplace page links the repo |
| "what licence is this, which version am I on?" | the Marketplace page; the version only in the Extensions view |

## 2. The two commands

| Command | Title (palette: `V-NOC: …`) | Does |
|---|---|---|
| `vnoc.reportIssue` | **Report Issue…** | §3 |
| `vnoc.help` | **Help and Feedback…** | §4: a QuickPick of every link |

The links live in **one file**, `extension/src/links.ts`: the repo URL, the issue form, Discord, the licence, the changelog at the version's tag.
Nothing else in the code spells a URL. (If D1 moves the repo, this file and `package.json` are the only code that changes.)

## 3. Report Issue

### 3.1 The flow

1. The command collects the facts of §3.3.
2. A **modal** message: *"Report an issue on GitHub. Your V-NOC, VS Code and system versions are filled in. The last 200 lines of the V-NOC log
   can be copied to your clipboard for you to paste into the form; they can contain file and function names from your project."*
   Buttons: **Open Form and Copy Log** · **Open Form** · (Cancel).
3. `vscode.env.openExternal(<the form URL>)`; with the first button, the log tail is put on the clipboard first.

**Nothing is sent by V-NOC.** The browser opens a GitHub form; the user reads it, edits it, and decides whether to submit. That is why this
needs no privacy setting and no telemetry rules.

### 3.2 The form URL

A GitHub **issue form** (02 §5, `.github/ISSUE_TEMPLATE/bug_report.yml`) can be pre-filled by query parameters named after its field `id`s:

```
https://github.com/<owner>/<repo>/issues/new?template=bug_report.yml
  &title=<empty: the user writes it>
  &vnoc=0.2.0 (core 0.2.0)
  &vscode=1.146.0
  &os=macOS arm64 (Darwin 23.6.0)
```

The log does **not** go in the URL: URLs over about 8 KB fail, and a log is long and private. It goes on the clipboard (§3.1), and the form's
*Log* field says *"Paste the log here (Report Issue copied it)."*

### 3.3 What is filled in

| Field id | Value | From |
|---|---|---|
| `vnoc` | the extension's version and the core's | `context.extension.packageJSON.version`; the core's `serverInfo.version` from `initialize` (`crates/vnoc/src/server.rs:253`), *not running* if it is not |
| `vscode` | `vscode.version` and `vscode.env.appName` (VS Code, Cursor, VSCodium… matters for Open VSX users) | |
| `os` | `process.platform`, `process.arch`, `os.release()` | |

### 3.4 The log tail

The output channel cannot be read back through the VS Code API. So `extension.ts` wraps the channel it makes (`createOutputChannel("V-NOC")`,
`extension.ts:43`) in a small **tee**: an object with the `OutputChannel` methods that forwards every call to the real channel and keeps the
**last 200 lines** in a ring buffer. That same object is what `core.ts` passes to the language client as `outputChannel` (`core.ts`, the
`LanguageClient` options), so the core's and the driver's stderr land in the buffer too. **[check]** that the client writes through
`append`/`appendLine` only.

## 4. Help and Feedback

A QuickPick. Title: **V-NOC 0.2.0 · core 0.2.0 · Apache-2.0**. Items, in this order:

| Item | Icon | Does |
|---|---|---|
| Report Issue… | `$(bug)` | §3 |
| Show Log | `$(output)` | `vnoc.showLog` |
| Ask on Discord | `$(comment-discussion)` | opens the Discord invite |
| Star V-NOC on GitHub | `$(star)` | opens the repo page. Description: *"It helps other people find it."* |
| Read the Docs | `$(book)` | the repo's README |
| What's New in 0.2.0 | `$(note)` | the CHANGELOG at the tag `v<version>` |
| Licence: Apache-2.0 | `$(law)` | `LICENSE` on GitHub; second line: *"Includes third-party code: see THIRD_PARTY_NOTICES"* (README R3) |

**The star: never a pop-up** (D3). It is one line in this list and one line in the README. A notification asking for a star in an editor that is
trying to help you work reads as begging, and Marketplace reviews punish it.

## 5. Where they appear

| Place | What | Note |
|---|---|---|
| the palette | both commands | always |
| the **STRUCTURE** view title, `⋯` | **Help and Feedback…** in a last group `9_help` | one entry, not five: 16/06 §9 keeps the title small |
| the **sidebar's error state** (16/06 §8) | Try Again · Show Log · **Report Issue** | `webview/src/sidebar/States.tsx`: a third button, a new request `reportIssue` to the host, like `showLog` |
| **the core stops for good** | an error notification: *"V-NOC stopped: <the reason>."* **Restart** · **Show Log** · **Report Issue** | where `core.ts` calls `showErrorMessage` today (start failed), and when the language client gives up restarting. **Once per run**, not on every retry |
| VS Code's **Help: Report Issue** | V-NOC in its list, posting to our repo | free from `bugs.url` (§6); our own command is still better (it fills the fields) |
| the Marketplace page | *Issues*, *Repository*, *Q&A* links | §6 |

Not in the canvas, not on cards, not in the editor menus: none of them are about the code.

## 6. `package.json`

```jsonc
"bugs": { "url": "https://github.com/yaredtsy/vnoc/issues" },
"homepage": "https://github.com/yaredtsy/vnoc#readme",
"qna": "https://discord.gg/J5nfPHqyBr",        // the Marketplace "Q&A" link; or Discussions, if you enable them (02 §7)
"galleryBanner": { "color": "#1e1e1e", "theme": "dark" },
"keywords": ["docs", "graph", "python", "documentation", "canvas", "call graph", "call hierarchy", "code visualization", "code navigation"]
```

The two commands go in `contributes.commands` (category `V-NOC`), the `⋯` entry in `menus.view/title` with `"when": "view == vnoc.structure"`.
Marketplace allows at most **30 keywords** **[check: vsce warns over 30]**.

## 7. What is not in this

**Telemetry, Sentry, any automatic sending.** Decided for 0.2 (D5). When it comes, it is its own step and its own doc: opt-in, behind
`vscode.env.isTelemetryEnabled` and `createTelemetryLogger`, file paths and names scrubbed, a `PRIVACY.md`, a README line. Until then the
answer to "how often does it crash?" is the issue tracker.

## 8. Tests

| Test | Where | Checks |
|---|---|---|
| `reportUrl(facts)` is a pure function | `extension/src/report.test.ts` (the `menus.test.ts` style, run by `pnpm --filter vnoc unit`) | encodes every value; never longer than 8,000 characters; **every query key is an `id` in `.github/ISSUE_TEMPLATE/bug_report.yml`** (the test reads the YAML, so a renamed field fails here and not in a user's browser) |
| the tee | same file | 250 lines in → the last 200 out, in order; `append` without a newline joins the last line |
| the commands exist | the window suite | both are registered; `vnoc.help`'s items are the 7 of §4 (open the QuickPick, read its items, close it) |
| the `⋯` entry | `menus.test.ts` | `vnoc.help` in `view/title` under `9_help` for `vnoc.structure` |

By hand: Report Issue opens the form with three fields filled; the clipboard has the log; the error state's Report Issue does the same (stop the
core binary to get there: set `vnoc.server.path` to a missing file).

## 9. The docs this step updates

- `16-ui-redesign/01-layout.md` §2: the row **J9**.
- `16-ui-redesign/06-sidebar.md` §8 (the error state's third button) and §9 (the `⋯` entry).
- `16-ui-redesign/08-menus.md`: the view title's `⋯` list.
- `extension/README.md`: a *Feedback* section (02 §2).
