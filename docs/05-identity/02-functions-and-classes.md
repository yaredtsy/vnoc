# 05.02 Functions and classes: the ID in the docstring

```
def markers
├── 1. 0.1: the docstring ID: line           (decided, F1)
├── 2. the price of the docstring
├── 3. later (0.4): a comment style as an option
│   ├── what it looks like
│   ├── what to reserve NOW so it is a pure addition
│   └── switching styles
└── 4. when the marker goes missing
```

## 1. 0.1: the docstring `ID:` line

```python
class OrderService:
    @retry(3)
    def parse_order(self, raw):
        """Parse the raw payload.

        ID: 0Q8V5W4T2F7KX9M3B6D1RZ4H7K
        """
        ...
```

| Case | What the driver does |
|---|---|
| the def has a docstring | add the `ID:` line before the closing quotes |
| no docstring | insert a new one: `"""ID: 0Q8V…"""` |
| the def (or its docstring) does not parse | **nothing**; the core attaches by name instead (`id_in_code = "no"`) |
| the docstring already has some other `ID:` text | that is the user's; V-NOC adds its own line |

All of this is pylspt's existing behaviour (`plan_ids`, `doc_slot`), and the wire message is `vnoc/planIds` in
`plan_vs/07-drivers/01-driver-protocol.md` §5. The remaining work is porting the spike S3 code into `drivers/python` (review D3).

**Reading** is tolerant (`ID: x`, `ID:x`, extra spaces, lower case). **Writing** is always the one canonical form.

## 2. The price of the docstring

Worth knowing, because users will ask:

- The line is part of `__doc__`. It shows in `help()`, in hover text in editors, and in generated docs (Sphinx, pdoc).
- Linters that check docstring style (pydocstyle, ruff's `D` rules) may complain about a section they do not recognise **[check with ruff's default rules]**.
- A function with no docstring gets one that contains only an ID. Some teams will not like that in a review.

None of these is a bug, and the old V-NOC worked the same way. They are the reason the second style below is worth having as an option.

## 3. Later: a comment style, as an option

You said: docstring only for now, and later make it configurable between docstring and a comment. Here is how to make that a pure addition.

### What it looks like

```python
class OrderService:
    # vnoc: 0Q8V5W4T2F7KX9M3B6D1RZ4H7K
    @retry(3)
    def parse_order(self, raw):
        """Parse the raw payload."""
        ...
```

- One comment line **directly above the def, above its decorators**, at the def's indentation.
- It does not touch `__doc__`. It looks the same in every language (`// vnoc: …` in TypeScript, Rust, Go), which makes later drivers simpler.
- Its weakness: tools that move or regenerate a function (some refactorings, some AI edits) are more likely to drop a comment above it than a line inside
  its docstring.

### What to reserve now

Three small things in 0.1 make the later addition free. None of them is work beyond a line of text.

| Reserve | Where | Why |
|---|---|---|
| The key **`[ids] style`** in `vnoc.toml`, values `"docstring"` (default) and `"comment"` | 0.1 does not read it; just do not use the name for anything else | the setting must be shared by the team, so it belongs in `vnoc.toml`, not in a VS Code setting |
| The comment text **`vnoc: <ID>`** | documented in the driver protocol as reserved | so no driver invents a different spelling in the meantime |
| A **`style`** field on `vnoc/planIds` requests, and capability flags `ids.style.docstring`, `ids.style.comment` | protocol doc | a driver says which styles it can write; the core never asks for one it cannot |

`node.toml` needs **nothing** new. `id_in_code = "yes"` means "there is a marker", whatever its style. The driver's outline already reports the ID it
found; it does not need to say how it was written.

### Switching styles

The rule that keeps this safe: **from the release that adds the comment style, every driver reads both styles, always.** The `style` setting only decides
what is *written* for new markers.

| Situation | Result |
|---|---|
| a project switches `docstring` → `comment` | existing docstring IDs keep working; new attaches write comments |
| a teammate has an older V-NOC that reads only docstrings | comment-marked nodes are found by their anchor instead ("attached by name"). Degraded, not broken. |
| "Convert all IDs to comments" | a bulk command with a preview, added whenever someone asks for it. Not needed for the option itself. |
| both styles on one def, same ID | fine |
| both styles on one def, different IDs | the duplicate rule in [`05-finding-nodes-again.md`](05-finding-nodes-again.md) §3 |

## 4. When the marker goes missing

Someone deletes the `ID:` line by hand, a docstring is rewritten, an AI agent regenerates the function.

In 0.1 this is quiet:

```
node.toml says id_in_code = "yes", but no marker with that ID is anywhere in the code
  └── is the anchor (path + name) still there?
        ├── yes → the node is Found "by name": outlined link icon, and a "Write ID again" action on the node
        └── no  → Missing
```

No pop-up and no automatic re-insert. Writing into someone's code a second time, after they removed the line, should be their click.
The grouped "3 IDs were removed · Restore / Keep removed" notice from the plan is a 0.4 item.

Next: [`03-files.md`](03-files.md)
