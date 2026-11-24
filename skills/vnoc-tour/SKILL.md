---
name: vnoc-tour
description: Builds a guided tour of code that the user plays step by step on the V-NOC canvas, with stops on functions, classes, files or folders, each with lit lines and one short paragraph. Use when the user asks to be walked through code or for a tour ("walk me through checkout_order", "give me a tour of app/", "explain step by step how a request reaches the database", "show me what you changed"); when a tour is open and the user says "this", "here", "explain that more" or "take me to …"; and when asked to continue, extend, pin or bring up to date an existing tour. Needs the vnoc command.
metadata:
  vnoc-version: "0.3.1"
---
# V-NOC tours

A tour is your explanation, cut into steps the user reads beside the code. You write it with `vnoc tour`; each stop shows on the
user's canvas the moment you add it. **The tour is the answer.** Do not also explain in the chat.

## The loop
1. `vnoc tour start --title "<what it is of>" --question "<what the user asked>" --author <your name>` prints the tour's id.
   Then say one line: `Tour started: <title>. It is on the V-NOC canvas.`
2. For each stop: read the code (a function with `vnoc code <node>`, a file or a folder with `vnoc show <node>`), then add the stop
   with its text as a heredoc: `vnoc tour add <id> <node> --body-file - <<'EOF'`, the text, and `EOF` on a line of its own.
   Search and read between stops as you need. Say nothing in the chat while you write.
3. Run `finish` on its own, after the last `add` printed "written": `vnoc tour finish <id>`. Then one line: `Tour finished: <title> · <n> stops.`

Add one stop per command, in the order the user should read them. Do not prepare the whole tour first.
One `vnoc tour` command per call, with the tour's id written out. No `T=…` or `$T`, no `for` loops, no two commands joined with `;` or a
newline: the host allows only a command it can read, and asks the user about anything else.

## The text of a stop
    <one paragraph: what this function is for>

    ## 11-24 <a title: what this part does, as a phrase>
    <one paragraph: why the code needs it>

    ### 11-16 <a detail inside lines 11-24>
    <one paragraph>

- General first, then detail. Most stops need two or three `##` blocks and no `###`. Use `###` only to take a long part apart.
- The line numbers are the file's, as `vnoc code` printed them.
- Say **why**, not what: the code is on screen beside your text. Never retell it line by line.
- A step is one short paragraph. No lists, no code blocks. `code` and *emphasis* are fine.
- A block's title says what the part does ("Retry only a dropped connection"), not the function's name.
- No block for a line that only starts or returns.
- A function under 8 lines is one step: the first paragraph only.
- A stop reached by a call from an earlier stop: add `--from <that stop> --line <the call's line>`. A file of a folder you
  explained, a method of a class: `--from <that stop>`.
- A function the tour already explained: `--same-as <that stop>`, with one sentence on why it is called again.
  A stop planned as a second visit (shown as `= s2` by `show` and `route`) takes `--same-as s2` too: the plan does not do it for you.

## When a command refuses
It lists what is wrong (`block 2 text: 412 of 280 characters`). Fix exactly that and send the stop again.
If it says the user **stopped** the tour: add nothing more, say the one line it gives you.

## When the user talks about the tour
"Explain this more", "why is that there", "what about here": run `vnoc tour where` first. It prints the step the user is on, its
lines and its text. Answer about that. If where says no tour has been opened, ask which tour.

## How big
Up to about 12 stops, just write it. Bigger: ask the user one question first, with two or three sizes named by what each adds
("only checkout_order · and what it calls · and what those call"). `vnoc tour route <node> --depth 2` gives the sizes.

## Existing tours
`vnoc tour list` · `vnoc tour show <id>` (the outline and the ids, to continue one) · `vnoc tour check` (what went out of date;
fix a stop with `vnoc tour add <id> <node> --replace <stop> --body-file -` and the new text).

## After finish
`finish` may print lines that start with `ask the user`: the tour's stops have no IDs, or files are not committed so it is not
pinned. Do not act on them. Ask the user that question in one line each, with the files named, and run the command on the line
only after a yes. A project can set this to happen by itself; then `finish` did it and says so, and there is nothing to ask.
A line that says not pinned with no command after it is for you: put "not pinned" in your last line, and offer nothing.
When `finish` wrote the IDs and pinned by itself, your last line is the one it printed (`… · pinned at <hash>`).

## Only when the user asks
- `vnoc tour go <id> <stop>` moves the user's screen: when they ask to be taken somewhere.
- `vnoc tour ids`, `pin --commit`, `share`, `delete`, `edit`, `import`, `settings`: when they ask for exactly that.
  Never commit for a tour unasked, and never change `[tours]` in `.vnoc/vnoc.toml` yourself.

## More
`vnoc guide tour` has the full rules, the sizes and examples. Naming a node: `file.py:12` · `file.py::Class.method` · a path.
To find a node by name: `vnoc find <name>`.
If `vnoc` is not found, tell the user (VS Code: "V-NOC: Install `vnoc` Command in PATH") and explain in the chat instead.
