---
name: dqlrs headless worker
overview: Add dqlrs --serve (stdio and loopback --bind), a long-lived JSON-lines worker that reuses Session (no TUI) so notebooks can exec many statements on one connection.
todos:
  - id: flags-stub
    content: "Add --serve and --bind (loopback only, exclusive with -c); ping/shutdown on stdio and TCP"
    status: completed
  - id: envelope
    content: "Map StatementResult and EngineError to one JSON envelope per request"
    status: completed
  - id: exec-session
    content: "op exec through Session; allow listed meta; refuse watch/clear/shell/exit"
    status: completed
  - id: loop-polish
    content: "Shared framed loop for stdio and TCP; one exec at a time; no ~/.dql_history"
    status: completed
  - id: bind-tests
    content: "Ephemeral --bind 127.0.0.1:0, refuse 0.0.0.0, one client at a time"
    status: completed
  - id: local-smoke
    content: "Optional DynamoDB Local serve test when port 8000 is up"
    status: completed
  - id: docs
    content: "Document --serve and --bind in rust-docs and dqlrs --help"
    status: completed
isProject: false
---

# dqlrs headless worker

Canonical write-up: [`rust-plans/headless_worker.md`](../../rust-plans/headless_worker.md).

`--bind` is **v1** (loopback TCP, same JSON-lines as stdio). Progress events are not.

Notebook kernel attach is a follow-up PR; do not mix it into the Rust serve crate.
