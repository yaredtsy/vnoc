---
name: vnoc-tour
description: Builds a guided tour of code that the user plays step by step on the V-NOC canvas, with stops on functions, classes, files or folders, each with lit lines and one short paragraph. Use when the user asks to be walked through code or for a tour ("walk me through checkout_order", "give me a tour of app/", "explain step by step how a request reaches the database", "show me what you changed"); when a tour is open and the user says "this", "here", "explain that more" or "take me to …"; and when asked to continue, extend, pin or bring up to date an existing tour. Needs the vnoc command.
metadata:
  vnoc-version: "0.4.0"
---
# V-NOC tours

A tour is your explanation, cut into steps the user reads beside the code. You write it with `vnoc tour`; each stop shows on the
user's canvas the moment you add it. **The tour is the answer.**

## The loop
1. `vnoc tour start --title "<what the tour explains>" --question "<what the user wants to understand>" --author <your name>` prints the tour's id.
   Then say one line: `Tour started: <title>. It is on the V-NOC canvas.`
2. For each stop: read the code (a function with `vnoc code <node>`, a file or a folder with `vnoc show <node>`), then add the stop
   with its text as a heredoc: `vnoc tour add <id> <node> --body-file - <<'EOF'`, the text, and `EOF` on a line of its own.
   Search and read between stops as you need. Say nothing in the chat from start to finish, not even a short line beside an add.
3. Run `finish` on its own, after the last `add` printed "written": `vnoc tour finish <id>`. Then one line: `Tour finished: <title> · <n> stops.`
   After it, at most one more line, and only for what the user must know that the tour cannot say.

`--title` is a phrase a person would name the tour by ("How checkout survives a payment timeout"), never a path or a function's name.
`--question` is optional: what the user wants to understand. A request that only names code ("walk me through app/x.py") has none: leave `--question` out, never copy the request into it.
If the request names a reach ("only this", "and what it calls", "two levels") or a detail ("a quick tour", "a detailed tour"), use it: `--route <node> --depth 0|1|2` and `--detail quick|detailed`. Otherwise pick `--detail` from the question: a quick look → quick; learning or onboarding → detailed; else normal.

Add one stop per command, in the order the user should read them. Send one `add` at a time and read its answer before the next: never several in one message. The answer may be a refusal to fix, or Stop.
For a tour across files (a question, not one function's call tree), find the route first, then start with `--plan <the nodes in reading order>` and fill it with `add`, in that order:
`vnoc tour start --title "…" --question "…" --author <your name> --plan <node> <node> …`. Stops are not reordered afterwards with `--replace`. The plan fixes only the order: write each stop's text right after reading it. Otherwise do not prepare the whole tour first.
One `vnoc tour` command per call, with the tour's id written out. No `T=…` or `$T`, no `for` loops, no two commands joined with `;` or a
newline: the host allows only a command it can read, and asks the user about anything else.

## The text of a stop
    <what this function is for>

    ## 11-24 <a title: what this part does, as a phrase>
    <why the code needs it>

    ### 11-16 <a detail inside lines 11-24>
    <text>

- General first, then detail. Most stops need two or three `##` blocks and no `###`. Use `###` only to take a long part apart.
- The line numbers are the file's, as `vnoc code` printed them.
- Say **why**, not what: the code is on screen beside your text. Never retell it line by line.
- A step's text is a full paragraph of 2-4 sentences: why this part exists, what depends on it or would break without it, and anything surprising (a bug, a trap, a default). A quick tour: one sentence. A detailed tour: use its larger budget for the reasoning. Markdown: a set of things is a list or a table. A flow across stops or files is a ```` ```mermaid ```` flowchart, one per step, best in the first stop's intro of a tour across files. A code fence only for what is not on screen (an example call, a payload), never the stop's own code. No headings, HTML or images in a text.
- A block's title says what the part does ("Retry only a dropped connection"), not the function's name.
- No block for a line that only starts or returns.
- A function under 8 lines is one step: the first paragraph only.
- A stop reached by a call from an earlier stop: add `--from <that stop> --line <the call's line>`. A file of a folder, a method of a class: `--from <that stop>` alone, only for containment, never to chain stops that do not contain each other.
- A function the tour already explained: `--same-as <that stop>`, with one sentence on why it is called again.
  A stop planned as a second visit (shown as `= s2` by `show` and `route`) takes `--same-as s2` too: the plan does not do it for you.

## When a command refuses
It lists what is wrong (`block 2 text: 512 of 420 characters`). Fix exactly that and send the stop again.
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
pinned. Do not act on them. Ask the user as a question in your last message, ending in `?`, with the files named; run the command on the line only after a yes. A project can set this to happen by itself; then `finish` did it and says so, and there is nothing to ask.
A line that says not pinned with no command after it is for you: put "not pinned" in your last line, and offer nothing.
When `finish` wrote the IDs and pinned by itself, your last line is the one it printed (`… · pinned at <hash>`).

## Only when the user asks
- `vnoc tour go <id> <stop>` moves the user's screen: when they ask to be taken somewhere.
- `vnoc tour ids`, `pin --commit`, `share`, `delete`, `edit`, `import`, `settings`: when they ask for exactly that.
  Never commit for a tour unasked, and never change `[tours]` in `.vnoc/vnoc.toml` yourself.

## More
`vnoc guide tour` has the full rules, the sizes and examples. Naming a node: `file.py:12` · `file.py::Class.method` · a path; quote one with `<` or `>` in it (`'…<locals>…'`).
To find a node by name: `vnoc find <name>`.
If `vnoc` is not found, tell the user (VS Code: "V-NOC: Install `vnoc` Command in PATH") and explain in the chat instead.
