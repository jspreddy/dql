---
name: dqlrs headless worker
overview: Add dqlrs --serve (stdio and loopback --bind), a long-lived JSON-lines worker that reuses Session (no TUI) so notebooks can exec many statements on one connection. Plan only until reviewed.
todos:
  - id: flags-stub
    content: "Add --serve and --bind (loopback only, exclusive with -c); ping/shutdown on stdio and TCP"
    status: pending
  - id: envelope
    content: "Map StatementResult and EngineError to one JSON envelope per request"
    status: pending
  - id: exec-session
    content: "op exec through Session; allow listed meta; refuse watch/clear/shell/exit"
    status: pending
  - id: loop-polish
    content: "Shared framed loop for stdio and TCP; one exec at a time; no ~/.dql_history"
    status: pending
  - id: bind-tests
    content: "Ephemeral --bind 127.0.0.1:0, refuse 0.0.0.0, one client at a time"
    status: pending
  - id: local-smoke
    content: "Optional DynamoDB Local serve test when port 8000 is up"
    status: pending
  - id: docs
    content: "Document --serve and --bind in rust-docs and dqlrs --help"
    status: pending
isProject: false
---

# dqlrs headless worker

Canonical write-up: [`rust-plans/headless_worker.md`](../../rust-plans/headless_worker.md).

`--bind` is **v1** (loopback TCP, same JSON-lines as stdio). Progress events are not.

Implement only after that document is reviewed. Do not attach the Jupyter kernel in the first Rust PR.
