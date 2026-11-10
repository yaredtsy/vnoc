# 06.02 — Proto layout

`proto/` at the repo root is shared: Rust generates from it in `vnoc-proto/build.rs`, and the
frontend generates TS with `buf generate` (`@bufbuild/protoc-gen-es`, which generates messages
and service descriptors for connect-es v2 **[check]** plugin set for the version you pin).

```
proto/
├── buf.yaml                 # lint + breaking-change rules
├── buf.gen.yaml             # TS codegen for the frontend
└── vnoc/v1/
    ├── common.proto         # NodeRef, Node, NodeKind, Span, Status, Page, Theme
    ├── project.proto        # ProjectService
    ├── graph.proto          # GraphService (reads)
    ├── sync.proto           # SyncService (requests + WatchProject stream)
    ├── group.proto          # GroupService
    ├── document.proto       # DocumentService
    └── node.proto           # NodeService (user fields: theme, description)
```

## Services (first version)

| Service | RPCs | Replaces (Python REST) |
|---|---|---|
| `ProjectService` | `CreateProject`, `ListProjects`, `GetProject`, `DeleteProject`, `OpenProject`, `CloseProject`, `UpdateProjectSettings` | `/projects` |
| `GraphService` | `GetChildren(parent, page, kinds)`, `GetNodes(ids)`, `GetAncestors(id)`, `GetCallTree(entry, from_owner, depth, page)`, `GetCallers(function)` (via `reaches`) | `/code-elements`, `/calls`, tree endpoints |
| `SyncService` | `RequestSync(project, scope)`, `WatchProject(project) → stream SyncEvent`, `ListSyncReports(project)` | `/projects/{id}/resync`, Socket.IO progress |
| `GroupService` | `CreateGroup`, `MoveIntoGroup`, `MoveOutOfGroup`, `MoveGroup`, `UpdateGroup`, `DeleteGroup` | `/groups` |
| `DocumentService` | `CreateDocument`, `UpdateDocument`, `DeleteDocument`, `ListDocuments(target)`, `ListOrphanedDocuments` | `/documents` |
| `NodeService` | `UpdateTheme`, `UpdateDescription`, `ListTombstoned`, `EmptyTrash` | parts of `/code-elements` |

Deferred services (07-roadmap/03): `LogService`, `TestService`, `PlaygroundService`,
`ConversationService`, `TaskService`, `VersioningService`.

## Example messages

```protobuf
syntax = "proto3";
package vnoc.v1;

message Node {
  string id = 1;                 // "Function/449d…", validated server-side into NodeId
  NodeKind kind = 2;
  string name = 3;
  string parent_id = 4;
  string owner_id = 5;
  NodeStatus status = 6;
  optional Theme theme = 7;
  string description = 8;
  oneof data {
    FolderData folder = 20;
    FileData file = 21;
    ClassData class = 22;
    FunctionData function = 23;
    GroupData group = 24;
    CallData call = 25;
  }
  bool has_children = 30;        // so the tree can show an expander without another call
}

enum NodeKind {
  NODE_KIND_UNSPECIFIED = 0;     // proto3 rule: 0 must be "unknown"
  NODE_KIND_ROOT = 1;
  NODE_KIND_FOLDER = 2;
  // …
}

message GetChildrenRequest {
  string project_id = 1;
  string parent_id = 2;          // "Root/root" for the top level (R2)
  repeated NodeKind kinds = 3;   // empty = all
  PageRequest page = 4;
}
```

## Rules

- **Package `vnoc.v1`.** Breaking changes go to `v2`, never into `v1`.
- **Never reuse field numbers.** Removed fields become `reserved 7; reserved "old_name";`.
- **Every enum starts with `…_UNSPECIFIED = 0`.**
- **IDs are strings on the wire, `NodeId` inside.** Validation happens in `vnoc-api` (`TryFrom`).
- **Pagination is a message** (`PageRequest { size, cursor }` / `PageResponse { next_cursor }`),
  keyset-based (cursor = last id), not offsets.
- **CI runs `buf lint` and `buf breaking --against main`**, so accidental breaking changes fail the build.

Next: [`03-application-services.md`](03-application-services.md)
