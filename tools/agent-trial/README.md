# agent-trial

The trial of `docs/21-agent-cli/06-skill.md` §8 (step A14): six scenarios, text or XML, one headless agent session each.
`trial.py` is python3 with the standard library only: it parses the stream-json transcript and sets the run's environment exactly, which is awkward in sh.

**By hand only; not in CI; re-run only the scenarios a skill/rules/guide change touches** (06 §8 "Manual only"). No test calls it.

    cargo build --release -p vnoc            # and the driver's release build (drivers/python)
    tools/agent-trial/trial.py run claude 1 text          # agent, scenario 1-6, arm text|xml, [n], [--no-rules] [--model M]
    tools/agent-trial/trial.py score tools/agent-trial/runs/claude-s1-text-n1
    tools/agent-trial/trial.py batch claude --model sonnet   # the 14 runs of 06 §8, in order, one results file under runs/

A run is `runs/<agent>-s<scenario>-<arm>-n<n>/` (git-ignored): `work/` (a fresh copy of the sample, its own git repo, the agent's folder), `bin/` (this branch's `vnoc` and driver; scenario 6's PATH leaves it out), `transcript.jsonl`, `meta.json`, `score.json`.
Scenario 5's injection is the sample's own `handover-notes.md` on `billing/charge.py`. A run is capped at 30 turns or 10 minutes and recorded as `capped`, never retried. `batch` writes `runs/batch-<time>.json` and `.md`, and stops at a run that reached outside its run dir.
Agents: `claude` (`--model sonnet`) and `codex` (`gpt-reserve`, effort `low`, set per run with `-m` and `-c`; `~/.codex/config.toml` is never edited).
Codex differences, on purpose or forced: `--ignore-user-config` keeps the user's plugins and MCP servers out; `-c allow_login_shell=false` because a login shell rebuilds `PATH` from the user's zsh profile and finds their own `vnoc` first (and makes scenario 6 impossible); `--ephemeral` for no session files (it still writes caches and logs under `~/.codex`); its sandbox (`workspace-write`, no network) lets `rm` run inside `work/`, where Claude's allowlist refuses it; no turn cap, only the 10 minutes; its events name no model or effort, so meta.json records the flags; no dollar figure (plan usage), tokens in = `input_tokens` (cached included).
The scores that need judgment (scenarios 3, 5, 6) say `judged` and quote the lines.
