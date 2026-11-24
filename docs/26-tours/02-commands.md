# 26.02 The commands: `vnoc tour`

```
commands
├── 1. what an agent does with it     the loop, in one picture
├── 2. the list                       and who runs what unasked
├── 3. start
├── 4. add                            the text an agent sends · blocks in blocks · what is checked · what a refusal says
├── 5. finish · drop · resume         and what Stop looks like from the agent's side
├── 6. list · show · check            reading tours; checking all of them
├── 7. where · go                     the reader's place, read and moved from a terminal
├── 8. pin                            and --commit
├── 9. ids · edit · share · delete    the user's commands
├── 10. export · import               a tour as one JSON document, nested for a person; flat inside
├── 11. plan · route                  announcing the route; a route from the call tree
├── 12. what each prints              text, --json, exit codes
└── 13. guard rails
```

Every command is a thin handler over a function in `crates/vnoc/src/tours/` (21's C2). `<node>` is any form 21.02 takes
(`file:line`, `file::Name`, an ID, a path). `<tour>` is a tour's id or the start of one ([`01`](01-on-disk.md) §5).

## 1. What an agent does with it

```
> walk me through checkout_order

vnoc tour start --title "checkout_order" --question "walk me through checkout_order" --author claude-code
      → started checkout-order-7k2q

vnoc code app/checkout.py::checkout_order                       # read
vnoc tour add checkout-order-7k2q app/checkout.py::checkout_order --body-file - <<'EOF'
checkout_order turns an order request into a completed order: it holds the stock, takes the money, and records
the sale, in that order.

## 11-24 Hold the stock, then charge
Nothing is charged until the items are set aside, and nothing stays set aside if the charge fails.

### 11-16 Check the stock, then hold it
check_stock only looks; reserve_stock takes the units out of the count, so a second buyer cannot get them
while this payment runs.

### 18-24 Charge, and give the stock back if it times out
A timeout is the one failure it knows how to undo: cancel_checkout puts the units back, then the error goes up.

## 26-30 Record the sale
The order is created only after the money is taken, so there is never an order nobody paid for.
EOF
      → s1 written: 5 steps

vnoc code app/inventory.py::check_stock                         # read the next one; search, grep, calls as needed
vnoc tour add checkout-order-7k2q app/inventory.py::check_stock --from s1 --line 13 --body-file - <<'EOF'
check_stock answers one question: are there at least `quantity` units of this SKU? An unknown SKU counts as
zero. It reads and never changes the stock.
EOF
      → s2 written: 1 step
…
vnoc tour finish checkout-order-7k2q
      → finished: 10 stops, 24 steps · pinned at f4d6e15

● Tour finished: checkout_order · 10 stops. It is on the V-NOC canvas.

> wait, explain this part more                                  # later, in the same chat
vnoc tour where
      → checkout-order-7k2q · stop 3 of 10 · step 2 of 3 · reserve_stock app/inventory.py:21-22 "Refuse what is not there"
```

Each `add` is one stop: its intro and all its blocks, in one call. Between two `add`s the agent does whatever it needs. The person
sees `s1` on the canvas when the first `add` returns.

## 2. The list

```
write, step by step
  vnoc tour start  --title "…" [--question "…"] [--author <name>] [--detail quick|normal|detailed]
                   [--plan <node>…] [--route <node> [--depth <n>]]
  vnoc tour add    <tour> <node> --body-file <file>|-  [--from <stop> [--line <n>]] [--same-as <stop>] [--replace <stop>]
  vnoc tour add    <tour> --to <stop> [--under <block>] --body-file <file>|-      more blocks on a stop that is written
  vnoc tour finish <tour> [--ids | --no-ids] [--commit [-m "…"] | --no-commit]
  vnoc tour plan   <tour> <node>… [--from <stop>]
  vnoc tour drop   <tour> <stop>|<block>
  vnoc tour resume <tour>

read
  vnoc tour list   [--on <node>] [--check]
  vnoc tour show   <tour> [--text] [--check]
  vnoc tour check  [<tour>…] [--again]
  vnoc tour where  [<tour>]
  vnoc tour route  <node> [--depth <n>]
  vnoc tour export <tour> [--flat]

the user's (an agent runs these only when asked)
  vnoc tour go     <tour> next|prev|next-stop|prev-stop|back|cover|end|<stop>|<block>
  vnoc tour pin    [<tour>…] [--commit [-m "…"]]
  vnoc tour ids    <tour> [<stop>…]
  vnoc tour edit   <tour> [--title "…"] [--question "…"] [--detail …]
  vnoc tour share  <tour> [--private]
  vnoc tour delete <tour>…
  vnoc tour import <file>|- [--replace <tour>]
  vnoc tour settings [--ids always|ask|never] [--commit always|ask|never]
```

| Group | On the agent's allow list (no permission question) | Why |
|---|---|---|
| write, step by step | yes | writes only inside the tours folders; an added line can be dropped; nothing can be lost |
| read | yes | reads |
| the user's | **no** | `ids` and `pin --commit` touch the source and git's history; `delete`, `share`, `edit`, `import` change what you keep; `go` moves your screen; `settings` changes what happens by itself |

Every command works for a person at a terminal. The split is only about what an agent does **by itself** (D14, ask 4). One thing
crosses it on purpose: `finish` is in the first group, and with `[tours] ids` or `commit` set to `"always"` it writes IDs or commits.
That is the project having said yes in advance ([`01`](01-on-disk.md) §12).

## 3. `start`

| Argument | Does |
|---|---|
| `--title "…"` | **required.** What the tour is of, in a few words: usually the name of the node it starts at (`checkout_order`, `app/`), or a phrase (`From a request to an order`). At most 60 characters, one line. It becomes the id's slug and the list's first line |
| `--question "…"` | what the person asked, in their words. Shown on the cover and in the list; it is what tells two tours of one node apart (design 07 §3). At most 200 characters |
| `--author <name>` | as a doc's `author:` (21 C16). The skill tells the agent to pass its own name |
| `--detail` | `quick`, `normal` (default) or `detailed`: which text budget applies (§4) |
| `--plan <node>…` | start with these stops planned, in this order (§11) |
| `--route <node>` | start with the route `vnoc tour route` proposes for this node (§11) |

What it does, in order: find the root (C9) → **create `.vnoc/` if there is none** (D16: `vnoc.toml`, `.gitignore`, by the function `init`
uses) → make the id → take the lock → write the `tour` record with `create_new` into `.vnoc/local/tours/` → the plan, if any → print.

`start` needs the index only when it is given nodes. A bare `start` starts no driver.

## 4. `add`

One stop per call. The node is read from the index as it is now: its kind, its lines, its ID if it has one. The text comes from
`--body-file` (a file, or `-` for standard input), as `doc new` takes its body.

**The text an agent sends:**

```
<the intro: one paragraph. What this node is for.>

## <first>-<last> <title>
<one paragraph: what this part is for.>

### <first>-<last> <title>
<one paragraph: a detail of the part above. Its lines are inside that part's.>

## <first>-<last> <title>
<one paragraph>
```

| Part | Rule |
|---|---|
| the intro | everything before the first heading. Required. One paragraph |
| `## 11-24 Hold the stock, then charge` | a block: file line numbers as `vnoc code` printed them (`## 24` is one line), then the title |
| `### 11-16 Check the stock, then hold it` | a detail of the `##` block before it. One level of detail: a `####` is refused (*"blocks nest two deep: make this a ### or its own ##"*). The file's format does not have that limit ([`01`](01-on-disk.md) §3); it is the screen's |
| a block's text | the lines after its heading, up to the next heading. One paragraph |
| a paragraph | lines of text; a line break inside it is a space. A blank line inside a part is a second paragraph, and is refused |
| inside a text | `` `code` `` and `*emphasis*`. Nothing else (the table below) |

A stop with `--same-as` sends the intro only: one sentence on why the tour is here again.

**What is checked**, all before anything is written (D9). Budgets are in characters, by the tour's `detail`:

| Field | quick | normal | detailed |
|---|---|---|---|
| a block's title | 60 | 60 | 60 |
| a stop's intro | 160 | 280 | 480 |
| a block's text | 160 | 280 | 600 |
| the sentence of a `--same-as` stop | 120 | 120 | 160 |

| Check | The refusal says |
|---|---|
| a text or a title over its budget | `block 2 text: 412 of 280 characters` |
| a text over 64 KB (read before anything else) | `the text is 70000 bytes: the most one stop takes is 65536` |
| the text has an intro | `intro: there is none. Start with one paragraph on what this is for, before the first heading` |
| a block has text | `block 2 text: there is none. Every block is one paragraph on why those lines are there` |
| a part is one paragraph | `block 2 text: a blank line starts a second paragraph. A step is one paragraph` (`intro: …` for the intro) |
| a heading has line numbers, from 1, the first not after the last, and a title | ``block 1: the heading has no line numbers: write `## 11-24 Title` (`## 24 Title` for one line)``; `block 1: lines start at 1, not 0`; `block 1: lines 24-11 are backwards: the first line comes first`; ``block 1: the heading has no title: write `## 11-24 Title` `` (a `###` heading says `###` in its example) |
| a `###` has a `##` before it | `block 1: a detail needs a block before it: write a ## block first` |
| a block's lines are inside the node | `block 1: lines 40-44 are outside reserve_stock (19-31)` |
| a detail's lines are inside its block | `block 3 (18-26) is not inside block 1 (11-24)` |
| blocks beside each other do not overlap | `blocks 2 and 3 both light line 17` |
| a node under 8 lines has no blocks (W9) | `check_stock is 4 lines: a stop this short is one step. Put it all in the first paragraph` |
| a folder has no blocks; a `--same-as` stop has none | `app/ is a folder: it has no lines to light` |
| a `--same-as` stop has no blocks | `a --same-as stop is one sentence: it has no blocks` |
| a title is a phrase on one line, with no full stop at the end | `block 1 title ends with a full stop` |
| a text has no list, table, code fence, image, link or HTML, and no heading but the block headings | `block 2 text: a list. A step is one paragraph` |
| `--from` is a stop of this tour; `--line` is inside that stop's node | `--line 99 is outside checkout_order (11-30)` |
| `--same-as` is a written stop on the same node | `s2 is check_stock, not reserve_stock` |
| at most 12 blocks in a stop (details counted), 40 stops on the route | `this tour has 40 stops, the most one tour holds. Finish it and start a second tour` |
| at most 12 blocks in a stop, details counted (said at the first block over) | `this stop has 13 blocks, the most one stop holds is 12 (details count). Make the rest a second stop` |
| the node is in the index | the usual "no such node" with "did you mean" (21.02 §4) |

Blocks are numbered in the order they are written in the text (`block 3` is the third heading, at any depth). Every problem of one
call is listed, and nothing is written. The agent fixes the text and sends the stop again.

**Where the stop goes on the route:**

| Call | The stop |
|---|---|
| `add <tour> <node>` and the route has a **planned** stop on this node, not written | fills the first such stop: its id, its place, its `from` and `line` (the call's own `--from`, if given, wins) |
| `add <tour> <node>` otherwise | a new stop: after the last stop nested in its `--from` stop, or at the end of the route |
| `--replace s3` | the stop `s3`, where it stands, with a new text and new blocks. The node may be the same or another. This is how a stale stop is fixed |
| `--to s3` | no new stop: the blocks in the text (no intro) are added after `s3`'s blocks. With `--under s3.b1` they are details of that block, written as `##` |

`--from s1 --line 16` says the tour got here by the call on line 16 of `s1`'s node. The route nests the stop under `s1`, the stage says
`↳ called on line 16`, and the map draws the edge. `--from s1` with no `--line` is containment: a file of the folder `s1`, a method of
the class `s1`. Without `--from` the stop is at the top level: right for a tour that is not one tree.

What is written: one `stop` record and one `step` record per block, in one write under the lock, each with its hash
([`01`](01-on-disk.md) §7). A tour that was pinned is not pinned after it; the output says so.

## 5. `finish`, `drop`, `resume`, and Stop

| Command | Does |
|---|---|
| `finish <tour>` | writes `end`. Planned stops still not written leave the route, and the output says how many. Then, by the project's settings (below): the IDs, then the pin. Finishing a finished tour is `0` and says so |
| `drop <tour> s3` · `drop <tour> s3.b2` | takes a stop (with its blocks, and the stops nested in it) or a block (with its details) off the tour. The ids are not reused |
| `resume <tour>` | after a Stop: writes `resume`, and `add` works again. For "continue the tour" |

**What `finish` does after `end`** ([`01`](01-on-disk.md) §12), in this order:

| | `"always"`, or the flag `--ids` / `--commit` | `"ask"` (the start) | `"never"`, or `--no-ids` / `--no-commit` |
|---|---|---|---|
| **1. IDs** (`[tours] ids`), when a stop's node has none | writes them, as `vnoc tour ids` does (§9), and names the changed files | prints `4 stops have no ID: the tour will not follow a rename. To write them: vnoc tour ids <tour>` | nothing |
| **2. the pin**, when everything is committed | pins | pins | pins |
| **2. the pin**, when files are not committed (`[tours] commit`) | commits exactly those files (§8) and pins | prints the files and `vnoc tour pin <tour> --commit` | prints `not pinned` and the files, with no command |

```
$ vnoc tour finish checkout-order-7k2q                       # both settings "ask"
finished: 10 stops, 24 steps
ask the user  4 stops have no ID: the tour will not follow a rename.        vnoc tour ids checkout-order-7k2q
ask the user  not pinned: 2 files have changes that are not committed       vnoc tour pin checkout-order-7k2q --commit
                M  app/inventory.py
                ?? app/gateway.py
```

A line that starts with `ask the user` is for the agent to pass on as a question, not to act on ([`04`](04-skill.md) §3).

**Stop, from the agent's side.** The reader pressed `■`; the core wrote `halt`. The agent's next `add`, `plan`, `drop` or `finish` on
that tour is refused, exit `1`:

```
stopped: the reader stopped this tour (6 stops written). Add nothing more.
Say one line: "Tour stopped at stop 6."
To go on later, only if the user asks: vnoc tour resume checkout-order-7k2q
```

and the core writes `halted`, which is how the canvas knows the agent has been told (design 02 frame 8). A command cannot interrupt an
agent in the middle of reading; it is answered at its next write, at most one stop later.

## 6. `list`, `show`, `check`

```
$ vnoc tour list
checkout-order-7k2q   writing    6 of 10 stops   private                        checkout_order   "what happens when the payment times out"
app-3fd0              finished   9 stops         project   pinned 8a1c2e4        app/
reserve-stock-m3xd    finished   1 stop          private   not pinned · changed  reserve_stock

$ vnoc tour show checkout-order-7k2q
checkout_order   "what happens when the payment times out"
private · writing · 6 of 10 stops · 15 steps · by claude-code · not pinned (being written)
 1  s1  ƒ checkout_order      app/checkout.py:11-30     5 steps
 2  s2    ƒ check_stock       app/inventory.py:12-15    1 step    ← s1 line 13
 3  s3    ƒ reserve_stock     app/inventory.py:19-31    3 steps   ← s1 line 16
 4  s4      ƒ check_stock     app/inventory.py:12-15    1 step    ← s3 line 21   = s2
 …
 7  s7    ƒ cancel_checkout   app/checkout.py:33-40     planned   ← s1 line 21

$ vnoc tour check
checkout-order-7k2q   fresh           (checked now)
app-3fd0              fresh           (nothing changed since 18:52)
reserve-stock-m3xd    1 stop changed  (checked now)
  s1 changed: reserve_stock app/inventory.py:19-33 · s1.b2 lines 24-28 are no longer there
3 tours · 1 with stale stops
```

| Command | Notes |
|---|---|
| `list` | newest first. The last word of a row is the **recorded** check, if there is a valid one (`changed`, or nothing); `list` itself never checks. `--check` checks first. `--on <node>` keeps the tours that pass through the node and says how (starts here · stop 3 of 10 · 4 stops inside), as the list's second view does ([`03`](03-core.md) §6) |
| `show` | the outline, with the ids an agent needs to go on: this is how a tour is **continued in a new session**. Details are indented under their block. `--text` prints every intro, title and text under its row. `--check` marks each stop `changed`, `moved` or `gone` |
| `check` | **with no tour named: every tour.** For each, the recorded answer if nothing changed since (a `stat` per file), else a new check, which is recorded ([`01`](01-on-disk.md) §9). `--again` checks all of them anew. Also says `can be pinned` / `can be pinned again` (§8). For an agent asked to bring a tour up to date: `check`, read the code again, `add --replace` each stale stop. Exit `0`: a stale tour is not a failed command |

## 7. `where` and `go`: the reader's place (D17)

The place is kept by the core, in the tour's state file ([`01`](01-on-disk.md) §9). Both commands are one-shots that read or write
that file; the running core sees the change through the watcher, as it sees a new stop.

```
$ vnoc tour where
checkout-order-7k2q   "checkout_order"   open on the canvas · moved 12 s ago
stop 3 of 10 · step 2 of 3     s3.b1
ƒ reserve_stock   app/inventory.py:21-22
  Refuse what is not there
  It checks the stock again by itself. checkout_order already did, but a caller that skips the check must not
  push the count below zero.
  21  if not check_stock(sku, quantity):
  22      raise ValueError(f"Not enough stock for {sku}")
before: s3 (intro) · after: s3.b2 "Take the units, warn when few are left"
```

| Command | Does |
|---|---|
| `where` | the place of the tour **whose place moved last**: the tour, the stop's number, the step, the node and the lit lines with their code, the title and the text, the steps before and after. It says whether the tour is open on a canvas now, and how long ago the reader moved, so an agent can tell "they are looking at this" from "they were here yesterday" |
| `where <tour>` | the same, for that tour |
| `go <tour> next` · `prev` · `next-stop` · `prev-stop` · `back` · `cover` · `end` · `s4` · `s3.b2` | moves the place, by the same function the canvas's Next uses ([`03`](03-core.md) §4), and prints the new place as `where` does. A canvas showing that tour follows. A canvas that is not showing it is not opened or brought forward (W11): the place is simply there when the tour is next looked at |

What it is for:

- **"Explain this more"** in a chat. The agent runs `where`, knows what "this" is, and answers. (In 27 the answer becomes a step on the
  tour instead of chat text; `where` is the same.)
- **"Take me to where the payment is retried."** The agent finds the stop (`show`), runs `go`.
- A test of the screen, and of the core, that needs no clicks.

`where` is on the allow list; `go` is not (ask 4): moving someone's screen is done when they ask.

## 8. `pin` (D19)

[`01`](01-on-disk.md) §8 has what a pin is. The command:

```
$ vnoc tour pin reserve-stock-m3xd
not pinned: 2 files of this tour have changes that are not committed
  M  app/inventory.py
  ?? app/gateway.py
commit them, then run this again. Or: vnoc tour pin reserve-stock-m3xd --commit

$ vnoc tour pin reserve-stock-m3xd --commit -m "Stock: refuse what is not there"
committed 2 files as 3c9e1f7   Stock: refuse what is not there
  app/inventory.py
  app/gateway.py
pinned reserve-stock-m3xd at 3c9e1f7
```

| Call | Does |
|---|---|
| `pin <tour>…` | the three questions of 01 §8. Pinned, or not pinned with the reason: stale stops (by id), files not committed (as `git status` shows them), a file git ignores, no git |
| `pin` with no tour | every finished tour that is not pinned, or whose pin cannot be reached from HEAD: pins the ones that can be, lists the others with their reason |
| `--commit` | when the only thing in the way is files that are not committed: `git add` the untracked ones among them, `git commit --only -m <message> -- <those files>`, then pin. **Exactly the tour's files, the whole of each, nothing else:** other staged or changed files stay as they were. Refused when a stop is stale (commit would pin the wrong code), in the middle of a merge or a rebase, and when a file is ignored. A hook that fails the commit fails the command, with the hook's output |
| `-m "…"` | the message. Without it: `Code for the tour "<title>"` |

`finish` runs the plain `pin` and prints its result. Whether it also commits is the setting `[tours] commit` (§5): by default it does
not, and the skill tells the agent to **ask the user** and to run `pin --commit` only on a yes.

## 9. `ids`, `edit`, `share`, `delete`

| Command | Does |
|---|---|
| `ids <tour> [<stop>…]` | gives every stop's node (or the ones named) an ID, where it has none: the ID, the marker in the code, `node.toml`, by the attach flow without its doc. Then one `ids` record. Prints each file it changed, and `read these files again before you edit them`. Lines move down by one where a marker was added; the hashes leave marker lines out, so the tour stays fresh. Refused in a project whose `vnoc.toml` says `[ids] default = "none"`. A node that already has an ID is only recorded |
| `edit <tour> --title "…" --question "…" --detail …` | a `meta` record. The id and the file's name stay |
| `share <tour>` | moves a tour that is **not being written** (finished, or stopped) to `.vnoc/tours/` and prints `commit it to share it with the project`. A tour that is being written is refused. `--private` moves it back |
| `delete <tour>…` | moves the tour's file and its state file to `.vnoc/local/trash/` and prints the path. Nothing is erased. A tour that is being written is refused (stop it first) |
| `settings` | prints the two settings of [`01`](01-on-disk.md) §12 and where each value comes from (the file, or the default). With `--ids <value>` or `--commit <value>`: changes that key in `.vnoc/vnoc.toml` and keeps everything else in the file, as `vnoc tag describe` does for `tags.toml` |

A tour's **text** is changed by replacing its stop (`add --replace`), or as a document (§10). There is no command per word.

## 10. `export` and `import`: a tour as one document (D20)

V-NOC keeps a tour flat ([`01`](01-on-disk.md) §3). A person gets it **nested**, because nesting is how you see the shape when you
read a file: a stop's `stops` are the stops inside it, a block's `blocks` are its details. `export` turns the flat lists into the tree;
`import` turns the tree back into flat records. Nothing nested is ever stored.

```json
{
  "schema": 1,
  "title": "checkout_order",
  "question": "what happens when the payment times out",
  "author": "claude-code",
  "detail": "normal",
  "stops": [
    { "id": "s1", "node": "app/checkout.py::checkout_order",
      "text": "checkout_order turns an order request into a completed order: …",
      "blocks": [
        { "id": "s1.b1", "lines": [11, 24], "title": "Hold the stock, then charge", "text": "…",
          "blocks": [
            { "id": "s1.b2", "lines": [11, 16], "title": "Check the stock, then hold it", "text": "…" },
            { "id": "s1.b3", "lines": [18, 24], "title": "Charge, and give the stock back if it times out", "text": "…" } ] },
        { "id": "s1.b4", "lines": [26, 30], "title": "Record the sale", "text": "…" } ],
      "stops": [
        { "id": "s2", "line": 13, "node": "app/inventory.py::check_stock", "text": "check_stock answers one question: …" },
        { "id": "s3", "line": 16, "node": "app/inventory.py::reserve_stock", "text": "…", "blocks": [ … ],
          "stops": [
            { "id": "s4", "line": 21, "same_as": "s2", "node": "app/inventory.py::check_stock", "text": "Called again here: …" } ] } ] }
  ]
}
```

**The translation**, both ways, is one walk:

| In the document | In V-NOC's flat lists |
|---|---|
| a stop inside another stop's `stops` | that stop, with `from` = the stop it is inside |
| a block inside another block's `blocks` | that block, with `parent` = the block it is inside |
| the order: a stop, then the stops inside it, top to bottom | the route's order |
| the order of a stop's `blocks`, each followed by its own `blocks` | the stop's reading order |
| `line` on a nested stop | the line of the call in the stop it is inside |

| Key | Rule |
|---|---|
| `id` | optional in a new tour: a label of your own, needed only on a stop that a `same_as` names; the real ids are made in order. With `--replace`: an id the tour has means "this one" (its place and seen marks hold); anything else is new |
| `node` | any form a command takes (`file::Name`, `file:line`, an ID, a path) |
| `same_as` | the `id` of a stop that comes earlier in the reading order |
| `from`, `parent` | not needed: the nesting says it. `import` also takes the flat shape (`export --flat`), where they are the nesting. A document that says both and disagrees with itself is refused |

| Command | Does |
|---|---|
| `export <tour>` | prints the nested document for the tour as it is now (dropped things gone, replaced things as replaced). `node` is the name; a stop found by its ID is written with the name it has now. `--flat` prints V-NOC's own shape instead, for a script |
| `import <file>\|-` | **a new tour** from a document. `title` is required. Every stop is checked as `add` checks it, **all refusals of the whole document at once**, each with its place in the document: `stops[0].stops[1].blocks[0].text: 412 of 280 characters`. Nothing is written if any is refused. The tour is written finished, and what `finish` does after `end` (§5) is done |
| `import <file>\|- --replace <tour>` | **replaces that tour** with the document. A stop or block whose `id` is in the tour keeps it; one with no such `id` is new; one that is missing is gone. The same checks, against the code as it is **now**, and every hash is taken anew. The old file goes to the trash; the new one is written whole ([`01`](01-on-disk.md) §2). The tour is not pinned after it until pinned again |

- **The round trip:** `export`, edit, `import --replace`. With nothing edited, on a tour whose code did not change, it gives a tour
  that plays the same, with the same ids.
- An agent writes stop by stop (the skill says so): a document arrives whole, so nobody can read along, and one bad stop refuses all.
  `import` is for a person, a script, or a tour moved from another project.
- `export` is on the agent's allow list (it reads); `import` is not.

## 11. `plan` and `route`

A **plan** is stops that are on the route before they are written. It is optional. What it buys is on the screen: the whole outline at
once, "stop 3 of 10", a stop you can open and read the code of before its text exists (design 02 frames 2 and 6).

| Command | Does |
|---|---|
| `start --plan a b c` | starts the tour with three planned stops |
| `plan <tour> <node>… [--from <stop>]` | plans more; `--from` nests them under a stop |
| `add` on a planned node | fills it (§4). A stop the agent adds that was never planned simply joins the route |
| `finish` | removes what is still planned |

**`route`** is the phase documents' idea, kept as a help: the core proposes a route from the call tree, and says how big it is.

```
$ vnoc tour route app/checkout.py::checkout_order --depth 2
checkout_order                           1 stop    about 1 min
and what it calls                        6 stops   about 4 min      --depth 1
and what those call                     10 stops   about 7 min      --depth 2

 1  ƒ checkout_order      app/checkout.py:11
 2    ƒ check_stock       app/inventory.py:12     line 13
 3    ƒ reserve_stock     app/inventory.py:19     line 16
 4      ƒ check_stock     app/inventory.py:12     line 21   = 2
 …
```

- The first lines are the choices the design asks the agent to offer (W13): a route is named by **what it adds**, with stops and
  minutes. Minutes are steps × 20 seconds, with a guessed 2 steps per stop, rounded up: a guess that is the same guess every time.
- `start --route <node> --depth 2` plans exactly this: each stop with its `from` and `line`, and `same_as` on a node the route has
  already passed.
- It is `calls` underneath (`calls::merge`), in reading order, cut at 40 stops. A call the index does not know (a library) is not a stop.
- For a folder or a file, depth is containment: the folder, its files, their functions.

## 12. What each prints

Plain text by default, short; `--json` on every command, with `"schema": 1` and `"command": "tour add"` (C5).

**`start`:**

```
started checkout-order-7k2q   private   .vnoc/local/tours/checkout-order-7k2q.jsonl
created .vnoc/ (vnoc.toml, .gitignore)                                   ← only when it did
next: vnoc tour add checkout-order-7k2q <node> --body-file -             (vnoc guide tour)
```

**`add`** says what was lit, by the first line of each block, so the agent sees a wrong line number at once:

```
s1 written   stop 1   ƒ checkout_order   app/checkout.py:11-30   5 steps   · 1 of 10 stops written
  s1.b1  11-24   def checkout_order(order: OrderRequest) -> CompletedOrder:
    s1.b2  11-16   def checkout_order(order: OrderRequest) -> CompletedOrder:
    s1.b3  18-24   try:
  s1.b4  26-30   order_record = create_order(order, transaction_id)
```

`--json`: `{ "schema": 1, "command": "tour add", "tour": "checkout-order-7k2q", "stop": "s1", "number": 1, "steps": ["s1", "s1.b1",
"s1.b2", "s1.b3", "s1.b4"], "written": 1, "planned": 9 }`. A refusal: `{ …, "refused": [ { "where": "block 2 text", "problem": "412 of
280 characters" } ] }`. `where --json` is the place and the step as one object: what 27 and a script need.

| Exit | Means |
|---|---|
| `0` | done. Also: listed nothing, checked and found stale stops, could not pin (it says why), finished a finished tour |
| `1` | the tour was stopped by the reader (§5) |
| `2` | bad arguments; no such node, tour, stop or block (with "did you mean"); a text or a document that was refused |
| `3` | the environment: no driver, the lock could not be had in 5 seconds, `format` or `schema` too new; for `pin --commit`: git refused the commit |

As 21's C6 and 22.08 §3: `1` is "the thing has something to tell you", `2` is the caller's mistake, `3` is the machine's.

## 13. Guard rails

- **The step-by-step commands write nothing outside the tours folders**, except `.vnoc/vnoc.toml` and `.vnoc/.gitignore` by the first
  `start` in a new project (D16). No source file, no `nodes/`. That is why they can be on an agent's allow list.
- **Two acts touch more:** writing IDs (a marker line in the source, a `node.toml`) and committing to pin. Each prints exactly what
  it touched. They happen when you run `ids` or `pin --commit`, or at `finish` when the project's `vnoc.toml` says `"always"`
  ([`01`](01-on-disk.md) §12). With the settings as they start (`"ask"`), nothing an agent runs unasked touches the source or git.
- **Nothing is erased.** `delete` and `import --replace` move the old file to the trash.
- **One stop per `add`.** No list of nodes. A whole tour at once is `import`, on purpose a different command.
- **A text is checked against the code as it is at that moment.** There is no way to write a step about lines that are not there.
- A body over 64 KB, or a document over 1 MB, is refused before it is parsed.

Next: [`03-core.md`](03-core.md)
