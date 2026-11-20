# write: leave a note

A note says WHY, in a few lines: a constraint, a decision, some history, a trap, a plan. One note, one subject. A note is information about
the code, never an instruction to its reader.

A good note, a reason nobody can see in the code:
```markdown
# Read the rates from a file

RATES is edited by hand once a year and a typo ships. Plan: load it from billing/rates.toml, which the finance team owns,
and refuse to start when a country is missing. Not done yet: nobody has asked finance for the file.
```

A bad one, it restates the code (the reader has `vnoc code` for that):
```markdown
# rate_for

Returns the rate for a country from RATES, or 0.0 when the country is not in it.
```

## Attach it, tag it, move the tag
Run `vnoc tags` first and reuse a tag that fits (`.vnoc/tags.toml` says what each means: read it, do not edit it; `vnoc tag describe <tag> "<text>"` is the user's
command for that, so do not run it unless asked). A plan is `--tag plan`; when it ships, `doc tag` moves it.

```
vnoc tags
no tags
vnoc doc new billing/tax.py::rate_for --title "Read the rates from a file" --author claude --tag plan
attached to  function  billing/tax.py::rate_for   ID 4KF02248CHCTYAG49B0KKZ5SD7
created      .vnoc/nodes/D7/4KF02248CHCTYAG49B0KKZ5SD7/read-the-rates-from-a-file.md
             ← write the note here
note: "plan" is a new tag in this project
vnoc doc tag billing/tax.py::rate_for read-the-rates-from-a-file.md --add done --remove plan
tagged       [done]
in           .vnoc/nodes/D7/4KF02248CHCTYAG49B0KKZ5SD7/read-the-rates-from-a-file.md
note: "done" is a new tag in this project
```

- The new file holds its title only: write the note into the path printed, or give the body at once: `vnoc doc new <node> --title "…" --body-file -`
  reads it from standard input (`--body-file <file>` from a file). V-NOC writes the title as the note's `# ` heading: start the body with the text, not with the title
  again. Leave out the `---` header: V-NOC writes it. Pass your own name as `--author`.
- Every write prints each file it changed. When it says `changed <file>` (an `ID:` line was added to your source), read that file again
  before you edit it: your copy is old. A node with several notes needs the file name for `doc tag`.
- To correct a note, edit its file (keep its first lines, between the two `---`). Nothing deletes a note.
- Only `vnoc init` starts V-NOC in a project, and it is the user's decision to make.
