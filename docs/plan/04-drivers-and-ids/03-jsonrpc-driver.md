# 04.03 — Out-of-process drivers (JSON-RPC): TS/JS and future languages

The TS/JS driver (`v-noc/src/lsp/ts_js`, Bun + ts-morph + Hono) stays a separate
process (D2, D7). P3 proposes keeping its JSON-RPC-over-HTTP protocol, but
**batched** (v2 methods).

---

## Process lifecycle (`JsonRpcDriver`)

Same contract as Python's driver manager and pylspt's transport plan:

```
spawn: <command> --host 127.0.0.1 --port 0
read stdout until:  READY port=<n>        (timeout 20 s)
serve: POST http://127.0.0.1:<n>/rpc      JSON-RPC 2.0
```

- Spawned with `tokio::process::Command`, `kill_on_drop(true)` so a backend crash doesn't leave zombies.
- stdout after `READY` and stderr are forwarded into `tracing` with a `driver=ts` field.
- **Health**: `health` RPC every 10 s while a project is open; restart on 3 failures (with backoff).
  Python's `is_alive()` only checked the client object (L5 analysis, section 6). This one checks the process.
- **Restart is safe**: the driver is stateless from our point of view (its cache rebuilds). A sync running
  during a crash fails only the files of the in-flight call (treated as unchanged).

## v2 methods (batched)

| Method | Params | Result |
|---|---|---|
| `initialize` | `{ project_root, settings }` | `{ language, extensions }` |
| `files_changed` | `{ changes: [{path, kind}] }` | `{}` |
| `outline` | `{ files: [path] }` | `{ outlines: [FileOutline] }` |
| `call_graphs` | `{ entries: [EntryRef], opts }` | `{ graphs: [EntryGraph] }` |
| `read_ids` / `assign_ids` | see 04-id-strategy | |
| `health`, `shutdown` | `{}` | `{status}` |

The JSON shapes are the serde form of the Rust types in 01, so `serde` does both sides of the
Rust end. For the TS end, generate TS types from the Rust types with `ts-rs` or `specta`
(🟡 proposed), or write a JSON Schema. Either way, there's one source of truth.

**Migration for the existing TS driver:** add the batched methods next to the old ones,
then remove the old ones once v2 is in. The call-graph output must be shaped like pylspt's
(contexts + rendered tree); until it is, the TS driver can return a tree only, and the adapter
derives a trivial layer 1 from it (one context per function, digest = hash of its children).

## Client (`client.rs`)

```rust
pub struct JsonRpcClient { http: reqwest::Client, url: Url, next_id: AtomicU64 }

impl JsonRpcClient {
    pub async fn call<P: Serialize, R: DeserializeOwned>(&self, method: &str, params: &P,
                                                          timeout: Duration) -> Result<R, DriverError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let body = json!({ "jsonrpc": "2.0", "method": method, "params": params, "id": id });
        let resp: RpcResponse<R> = self.http.post(self.url.clone()).timeout(timeout)
            .json(&body).send().await?.error_for_status()?.json().await?;
        resp.into_result()                       // Err(RpcError { code, message, data }) → DriverError::Rpc
    }
}
```

> **🦀 Rust note: `AtomicU64` for the request id.**
> Many tasks call the driver at once, so the counter is shared. An atomic integer
> can be incremented from many threads **without a lock**. `Ordering::Relaxed` is
> enough because we only need unique numbers, not ordering between threads.

> **🦀 Rust note: `DeserializeOwned` vs `Deserialize<'de>`.**
> `R: DeserializeOwned` means "R can be built from JSON without borrowing from the
> input buffer". We need that because the response body is dropped at the end of the
> function. If you see lifetime errors around `Deserialize<'de>`, this is usually the fix.

## Exit criteria (M11)

- The TS driver passes the same driver test suite as pylspt (on a TS fixture).
- Killing the Bun process mid-sync: the sync completes, the affected files are reported failed, the driver is restarted, and the next sync is clean.

Next: [`04-id-strategy.md`](04-id-strategy.md)
