# 20.02 Names

```
names
├── 1. what a node offers     name · qualified name · path
├── 2. the six tiers
├── 3. fuzzy by word          splitting a name · the match · why not a plain subsequence
├── 4. several words
├── 5. the order
└── 6. the ranges to draw bold
```

## 1. What a node offers

| Field | For `OrderService.parse_order` in `src/orders.py` | For the file |
|---|---|---|
| **name** | `parse_order` | `orders.py` |
| **qualified name** | `OrderService.parse_order` (the driver's `qualname`; none for files and folders) | — |
| **path** | `src/orders.py` (relative to the opened folder) | `src/orders.py` |

Case is ignored everywhere. The opened folder itself is not searched (it is not a node of the project), as today.

## 2. The six tiers

A smaller tier is better. Tiers 0, 1, 4 and 5 are today's four, in today's order; 2 and 3 are new.

| Tier | A word matches when | Example for `co` |
|---|---|---|
| 0 | the name starts with it | `count`, `config_path` |
| 1 | the name holds it | `discount` |
| 2 | **its letters start the name's words, in order** (§3) | **c**heckout_**o**rder |
| 3 | **the qualified name holds it** | `Recorder.save` (its name, `save`, does not) |
| 4 | the path starts with it | `core/…` |
| 5 | the path holds it | `billing/coupons.py` |

A word is given the **best** tier it reaches. Tier 3 is what makes `OrderService.par` find the class's methods: the word has a `.`, no name holds
it, the qualified name does.

## 3. Fuzzy by word (tier 2)

**Splitting a name into words.** At `_`, `.`, `-` and spaces (dropped); between a lower-case letter and an upper-case one (`checkoutOrder` →
`checkout` `Order`); before the last capital of a run that is followed by lower case (`HTTPClient` → `HTTP` `Client`); between letters and digits
(`sha256sum` → `sha` `256` `sum`). Empty words are dropped, so `__init__` is `init` and `_private` is `private`.

**The match.** The query word matches if it can be cut, in order, into **prefixes of the name's words**, each name word giving one prefix or
none.

| Query | Name | Match | Why |
|---|---|---|---|
| `chor` | `checkout_order` | yes | `ch` + `or` |
| `co` | `checkout_order` | yes | `c` + `o` |
| `cko` | `checkout_order` | yes (tier 1: the name holds it) | `checkout` holds `cko`; the better tier wins |
| `kor` | `checkout_order` | no | no word starts with `k`, and the name does not hold it |
| `hc` | `HTTPClient` | yes | `H` + `C` |
| `httpcl` | `HTTPClient` | yes (tier 0: the name starts with it) | the better tier wins |
| `abcd` | `ab_abc_d` | yes | nothing from `ab`, `abc` from `abc`, `d` from `d`. Taking `ab` from the first word leaves `cd` with no word to start: **the match must try again when the greedy cut fails** |
| `or` | `checkout_order` | yes (tier 1: the name holds it) | the better tier wins |
| `init` | `__init__` | tier 1 | the name holds it |

A plain subsequence (`ckor` anywhere in the name) is **not** a match: in 10,000 names it finds too much. VS Code's go-to-symbol matches
`camelCase` this way; the rows above are the test rows either way.

**How.** A small search with a memo over (letters used, word reached): for each name word from the current one on, try each prefix of it that the
remaining letters start with, longest first; succeed when the letters are used up. Among the ways that succeed, keep the one that **skips the
fewest name words before its last used word** (then the longest first prefix). Names are short; no limit is needed beyond the name's length.

Comparing ignores case **letter by letter** (`char::to_lowercase` on each side), never by lower-casing the whole name first: a few letters change
length when lower-cased, and the ranges (§6) must point into the name as written.

## 4. Several words

`order check`: **each word is matched alone** (§2); a node is a hit when **every** word matches something (its name, qualified name or path); the
node's tier is the **worst** of its words' tiers. So `order check` finds `check_order` and `checkout_order` (both tier 1: the name holds both
words), and `billing/ pay` finds `pay…` under `billing/` at tier 4 (the path word is the worst).

## 5. The order

1. the exact reference, if the query was one (01 §1);
2. the tier;
3. inside tier 2, fewer name words skipped (with several words, the sum over the words; a word not at tier 2 adds 0). A node whose tier is not
   2 has 0 skipped: a tier-4 node is not ordered by the skips of its tier-2 word;
4. the shorter name (closer to the whole name);
5. reading order (the tree's order: folders first, then files, each sorted as the Explorer sorts; defs in source order).

Filters do not change the order; they remove.

## 6. The ranges to draw bold

Each node in the answer has `match: [start, end][]`: the parts of its **name** that matched, sorted, merged where they touch, in **UTF-16 code
units** (Q4).

| Tier | Ranges |
|---|---|
| 0, 1 | where the word is in the name |
| 2 | one range per prefix used (`chor` in `checkout_order`: `[0,2]` and `[9,11]`) |
| 3, 4, 5 | none: the name did not match (the row shows the path, as today) |

With several words, the ranges of every word that matched the name are joined. A name with `é` (one UTF-16 unit, two bytes) and one with an emoji
(two units, four bytes) are test rows.
