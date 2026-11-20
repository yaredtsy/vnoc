# agent-trial

The trial of `docs/21-agent-cli/06-skill.md` §8 (step A14): six scenarios, text or XML, one headless agent session each.
`trial.py` is python3 with the standard library only: it parses the stream-json transcript and sets the run's environment exactly, which is awkward in sh.

    cargo build --release -p vnoc            # and the driver's release build (drivers/python)
    tools/agent-trial/trial.py run claude 1 text          # agent, scenario 1-6, arm text|xml, [n], [--no-rules] [--model M]
    tools/agent-trial/trial.py score tools/agent-trial/runs/claude-s1-text-n1

A run is `runs/<agent>-s<scenario>-<arm>-n<n>/` (git-ignored): `work/` (a fresh copy of the sample, its own git repo, the agent's folder), `bin/` (this branch's `vnoc` and driver; scenario 6's PATH leaves it out), `transcript.jsonl`, `meta.json`, `score.json`. `setup <run-dir>` makes the folder alone.
Only Claude Code is built (`codex` and `cursor` say A14b). The scores that need judgment (scenarios 3, 5, 6) say `judged` and quote the lines.
