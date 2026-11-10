# 06.01 — gRPC / Connect for the frontend (D3)

---

## How a browser talks gRPC

Browsers can't speak native gRPC: it needs HTTP/2 trailers, which `fetch` doesn't expose. There
are two browser-friendly protocols:

| Protocol | Server side in Rust | Browser client |
|---|---|---|
| **gRPC-Web** | `tonic` + `tonic-web` (mature) | `@connectrpc/connect-web` → `createGrpcWebTransport` |
| **Connect** protocol | a Connect-protocol server crate **[check]** maturity | `@connectrpc/connect-web` → `createConnectTransport` |

**P2 (proposed):** tonic + tonic-web, with connect-web's gRPC-Web transport on the frontend.
Same `.proto` files, generated TS clients, mature server. If a solid Rust Connect server shows up
later, switching only means changing the transport on both ends. The services and messages stay.

### What gRPC-Web can and can't do

| RPC kind | Browser support | We use it for |
|---|---|---|
| unary | ✅ | everything request/response |
| **server streaming** | ✅ | `WatchProject` (progress + graph-changed events) |
| client streaming / bidi | ❌ | nothing (design avoids them) |

Python used Socket.IO for push. Server streaming replaces it.

## Server setup sketch

```rust
// crates/vnoc-server/src/main.rs (sketch)
let app = vnoc_app::App::new(store, drivers, cfg.clone());      // services, open projects map

let cors = tower_http::cors::CorsLayer::new()
    .allow_origin(cfg.server.cors_origins_parsed())
    .allow_headers(tower_http::cors::Any)                        // grpc-web needs x-grpc-web, content-type, …
    .expose_headers([ "grpc-status", "grpc-message", "grpc-status-details-bin" ].map(HeaderName::from_static));

tonic::transport::Server::builder()
    .accept_http1(true)                                          // gRPC-Web runs over HTTP/1.1 too
    .layer(cors)
    .layer(tonic_web::GrpcWebLayer::new())
    .layer(tower_http::trace::TraceLayer::new_for_grpc())
    .add_service(ProjectServiceServer::new(api::ProjectApi::new(app.clone())))
    .add_service(GraphServiceServer::new(api::GraphApi::new(app.clone())))
    .add_service(SyncServiceServer::new(api::SyncApi::new(app.clone())))
    .add_service(GroupServiceServer::new(api::GroupApi::new(app.clone())))
    .add_service(DocumentServiceServer::new(api::DocumentApi::new(app.clone())))
    .add_service(tonic_health::server::health_reporter().1)
    .serve_with_shutdown(cfg.server.listen, shutdown_signal())
    .await?;
```

**[check]** exact builder/layer API against the tonic version you pin. tonic's API changes between
minor versions (e.g. how `tonic-web` is enabled, and where `prost` codegen lives).

## Implementing a service (and the `Stream` return type)

```rust
#[tonic::async_trait]
impl SyncService for SyncApi {
    type WatchProjectStream = Pin<Box<dyn Stream<Item = Result<pb::SyncEvent, Status>> + Send>>;

    async fn watch_project(&self, req: Request<pb::WatchProjectRequest>)
        -> Result<Response<Self::WatchProjectStream>, Status>
    {
        let id: ProjectId = req.into_inner().project_id.parse().map_err(invalid_arg)?;
        let (status, rx) = self.app.sync.subscribe(&id).await.map_err(to_status)?;   // watch + broadcast
        let first = futures::stream::once(async move { Ok(pb::SyncEvent::from(status)) });
        let rest  = tokio_stream::wrappers::BroadcastStream::new(rx)
            .filter_map(|r| async move { match r {
                Ok(ev) => Some(Ok(pb::SyncEvent::from(ev))),
                Err(_lagged) => None,          // slow client: skip; 05-orchestrator/10
            }});
        Ok(Response::new(Box::pin(first.chain(rest))))
    }
}
```

> **🦀 Rust note: `Pin<Box<dyn Stream …>>`.**
> tonic needs *one nameable type* for the stream. Our stream is built from combinators
> (`once(...).chain(...)`), whose real type is huge and unnameable. Boxing it as a `dyn Stream`
> gives it a name. `Pin` is required because async streams may hold self-references
> while suspended, and pinning promises they won't move in memory. In practice you write
> `Box::pin(stream)` and move on.

## Layer rule: proto types stop at `vnoc-api`

```
pb::MoveIntoGroupRequest  ──(vnoc-api: parse + validate)──►  NodeId, NodeId  ──►  app.groups.move_into(…)
                                                             (domain types)
```

Conversions live in `vnoc-api/src/convert.rs` as `TryFrom<pb::X> for DomainX` (fallible, because
strings must become valid `NodeId`s) and `From<DomainX> for pb::X` (infallible).

> **🦀 Rust note: `From` vs `TryFrom`.**
> `From` is for conversions that can't fail, and it gives you `.into()` for free.
> `TryFrom` returns a `Result`, so you write `NodeId::try_from(s)?`. Picking the right one
> documents whether a conversion can fail, and the compiler makes callers handle it.

## Errors

One function maps `AppError` → `tonic::Status` (02-workspace-and-crates/03, "API mapping").
Messages to the client are short. Details go to `tracing` with a request ID that's also returned in
the status metadata, so a bug report can point to the log line.

## Exit criteria (M10)

- The React app calls `ListProjects` and subscribes to `WatchProject` through connect-web's gRPC-Web transport from `http://localhost:5173`.
- `grpcurl -plaintext localhost:8000 list` works (reflection, dev only).

Next: [`02-proto-layout.md`](02-proto-layout.md)
