# 03.04 — The TerminusDB adapter (`vnoc-store-terminus`)

Everything Terminus-specific lives in this crate. If you find yourself writing
"woql" anywhere else, stop.

Endpoints below were checked against the Python client
(`v-noc/src/backend/app/db/terminus_client/mixins.py`, `document.py`, `remote.py`),
which talks to the same server. Anything not seen there is marked **[check]**.

---

## Module layout

```
crates/vnoc-store-terminus/src/
├── lib.rs            TerminusStore (impl GraphStore), TerminusGraph (impl ProjectGraph)
├── client.rs         thin HTTP client: auth, URLs, retries, error mapping
├── schema.rs         v2 schema as JSON (one function per class), + migration check
├── encode.rs         Node/Patch ↔ JSON-LD documents and WOQL fragments
├── woql.rs           a small typed WOQL builder (only what we use)
├── reads.rs          one function per ProjectGraph read method
├── apply.rs          WritePlan → transaction(s)  (small vs big plans)
└── versioned.rs      impl Versioned (branches, log, diff)
```

## 1. The HTTP client (`client.rs`)

URL scheme (from `mixins.py`):

| What | URL |
|---|---|
| documents | `POST/PUT/DELETE/GET {api}/document/{team}/{db}/local/branch/{branch}` |
| WOQL | `POST {api}/woql/{team}/{db}/local/branch/{branch}` |
| schema | same document URL with `graph_type=schema` |
| create DB | `POST {api}/db/{team}/{db}` |
| branch | `POST/DELETE {api}/branch/{team}/{db}/local/branch/{name}` |
| squash | `POST {api}/squash/{team}/{db}/local/branch/{name}` → returns `api:commit` |
| reset | `POST {api}/reset/{team}/{db}/local/branch/{name}` with the commit path |
| rebase | `POST {api}/rebase/{team}/{db}/local/branch/{name}` |
| optimize | `POST {api}/optimize/{path}` (path URL-encoded, e.g. `team/db/_meta`, `team/db/local/_commits`, `team/db/local/branch/main`) **[check]** the exact set of paths worth optimising |
| diff / apply | `POST {api}/diff/…`, `POST {api}/apply/…` |

Every write sends `author` + `message` (Python's `_generate_commit`) and may send the header
**`TerminusDB-Data-Version: <version>`** (Python passes it as `last_data_version`). Terminus
**rejects the write if the branch moved** since that version. That's a built-in
compare-and-set on the whole branch, and we use it (see `apply.rs`).

```rust
pub struct TerminusClient {
    http: reqwest::Client,         // clone is cheap: it's an Arc'd pool inside
    base: url::Url,                // http://127.0.0.1:6363/api/
    team: String,
    auth: Auth,                    // basic auth today; token later
}

impl TerminusClient {
    async fn woql(&self, db: &DbName, branch: &str, query: Value, commit: Option<&CommitMeta>,
                  data_version: Option<&DataVersion>) -> Result<WoqlResponse, StoreError>;
    async fn insert_docs(&self, db: &DbName, branch: &str, docs: &[Value], meta: &CommitMeta,
                         data_version: Option<&DataVersion>) -> Result<DataVersion, StoreError>;
    // replace_docs, delete_docs, get_docs, create_db, branch, squash, reset, optimize …
}
```

Error mapping (one function): connection/timeout/5xx → `StoreError::Unavailable` (retryable);
data-version mismatch → `StoreError::Conflict`; `api:DatabaseAlreadyExists`, `api:DocumentNotFound`, …
→ specific variants; other 4xx → `StoreError::Rejected { message, detail }` with the JSON body kept.

> **🦀 Rust note: `reqwest::Client` is an `Arc` inside.**
> Build it **once** and clone it freely. Each clone shares the same connection pool.
> Creating a new `Client` per request (a common beginner mistake) throws away
> keep-alive connections and TLS sessions, so every request pays the connection cost again.

## 2. Schema v2 (`schema.rs`)

Written as JSON (no Python-style schema DSL needed). Sketch:

```json
[
  { "@type": "@context", "@base": "terminusdb:///data/", "@schema": "terminusdb:///schema#" },

  { "@type": "Class", "@id": "Node", "@abstract": [],
    "name": "xsd:string",
    "parent": "Container", "owner": "Container",
    "status": "Status", "tomb_seq": {"@type":"Optional","@class":"xsd:unsignedLong"},
    "tomb_at": {"@type":"Optional","@class":"xsd:dateTime"},
    "last_path": {"@type":"Optional","@class":"xsd:string"},
    "created_at": "xsd:dateTime", "updated_at": "xsd:dateTime",
    "description": {"@type":"Optional","@class":"xsd:string"},
    "theme": {"@type":"Optional","@class":"Theme"} },

  { "@type": "Class", "@id": "Container", "@abstract": [] },
  { "@type": "Class", "@id": "Root",   "@inherits": ["Container"], "@key": {"@type":"Lexical","@fields":["key"]}, "key":"xsd:string" },
  { "@type": "Class", "@id": "Folder", "@inherits": ["Node","Container"], "@key": {"@type":"Lexical","@fields":["key"]},
    "key":"xsd:string", "path":"xsd:string" },
  { "@type": "Class", "@id": "File",   "@inherits": ["Node","Container"], "@key": …,
    "path":"xsd:string", "language":"xsd:string", "content_hash":"xsd:string",
    "outline_hash":"xsd:string", "size_bytes":"xsd:unsignedLong" },
  { "@type": "Class", "@id": "Function", "@inherits": ["Node","Container"], "@key": …,
    "qname":"xsd:string", "file":"File", "span":"Span", "fn_kind":"FnKind", "body_hash":"xsd:string" },
  { "@type": "Class", "@id": "Call", "@inherits": ["Node","Container"], "@key": …,
    "target":"Callable", "entry":"Callable", "call_count":"xsd:unsignedInt",
    "contexts": {"@type":"Set","@class":"xsd:string"}, "subtree_hash":"xsd:string", "depth":"xsd:unsignedShort" },
  { "@type": "Enum", "@id": "Status", "@value": ["Active","Tombstoned"] },
  { "@type": "Class", "@id": "Span", "@subdocument": [], "@key": {"@type":"Random"}, "sl":"xsd:unsignedInt", … }
]
```

Notes:

- **`@key: Lexical` on a `key` field** makes `@id` = `Function/<key>`, which matches our `NodeId`
  exactly. No ID translation table. **[check]** URL-encoding of keys with `/` (context keys);
  hex-encode them to be safe.
- **`Container`** is an abstract class that Root/Folder/File/Class/Function/Group/Call inherit, so
  `parent: Container` type-checks every legal parent in the DB itself.
- **Status flattened** into `status` + optional `tomb_*` fields instead of a subdocument:
  filtering `status = Active` becomes one triple, with no join through a subdocument.
- **Hashes as hex strings** (`xsd:string`). 32 chars, easy to read in the Terminus console.
- **Booleans**: Python stored `is_root` as a string because of an `xsd:boolean` issue.
  v2 doesn't need `is_root` at all (Root is a class). If a boolean comes up, test `xsd:boolean`
  round-trips in the contract suite before working around it.
- **Schema version**: a `SchemaInfo` doc with `version: 2`. `open()` checks it and refuses to
  open an unknown version (instead of half-working).

## 3. Encoding plans (`encode.rs`, `apply.rs`)

| Plan item | Terminus operation |
|---|---|
| `creates` | `InsertDocument` (full JSON-LD doc) |
| `patches` | for each set field: `DeleteTriple(id, field, old?)` + `AddTriple(id, field, new)`. **No full-doc replace.** With `expect: Parent(p)`, the pattern `triple(id, parent, p)` is matched first, so if it doesn't hold, the transaction does nothing and we report `PreconditionFailed` **[check]** how to detect "matched 0" (bindings count) |
| `deletes` | `DeleteDocument(id)` |
| `side` | `InsertDocument` / `UpdateDocument` (side docs are small and fully sync-owned, so replace is fine) |
| `state` | patch `SyncState.seq` with an `expect` on the old value, in the same transaction |

### Small plans → one WOQL transaction on `main`

All of the above go into one `And([...])` query, sent with the `TerminusDB-Data-Version` we
read at snapshot time. One HTTP call, one commit, atomic.

"Small" = under a size budget (start: ~5k operations or ~8 MB of JSON, tune in M2).

### Big plans → a sync branch, then squash

```
1. base = head(main)                                  (data version / commit id)
2. create branch  sync-<seq>  from base
3. apply the plan in chunks to sync-<seq>            (many commits, but only on the side branch)
4. squash sync-<seq>                → one commit C   (parent = base)
5. reset main to C                                    (main moves base → C in one step)
6. delete branch sync-<seq>
on any failure in 2–5: delete the branch. main never saw anything.
```

Why this is safe: our `ProjectWriter` actor (03) guarantees nothing else writes to `main`
between steps 1 and 5. If you ever allow external writers, replace step 5 with `rebase`
(and handle its conflict response). **[check]** squash/reset semantics on a recent
TerminusDB version in M2, with a test that kills the process between each step.

## 4. Reads (`reads.rs`)

Use WOQL `select` with only the needed variables, never `get_all_documents`.
Sketch for `snapshot_structure`:

```
select [id, kind, path, owner, parent_is_group, hash, outline, status] (
  or( isa(id, Folder), isa(id, File) )
  triple(id, path, path)
  triple(id, owner, owner)
  triple(id, parent, p)  ( isa(p, Group) → parent_is_group = true ; else false )
  opt( triple(id, content_hash, hash) ) …
  triple(id, status, status)
)
```

**[check]** TerminusDB's **GraphQL** endpoint as an alternative for UI reads. It generates
back-link fields for reverse edges (children via `parent`), which could make
`children()`/`ancestors()` simpler. Try both in M2 and keep whichever is faster/clearer.
It's an adapter-internal choice either way.

---

## Exit criteria (M2)

- `create_project_graph` creates the DB, the schema and `Root/root`. Opening an unknown schema version fails cleanly.
- The contract suite (06) passes against a real TerminusDB in docker.
- Benchmark: 100k-node graph: `snapshot_structure` < 1 s, `children(Root)` < 50 ms, a 50-op plan applies in one commit.
- Kill-test for big plans: kill the process after each step 2–5, and `main` is either at `base` or at `C`, never in between.

Next: [`05-terminus-best-practices.md`](05-terminus-best-practices.md)
