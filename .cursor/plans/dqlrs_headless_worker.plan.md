---
name: dqlrs headless worker
overview: Add dqlrs --serve, a long-lived JSON-lines worker that reuses Session (no TUI) so notebooks can exec many statements on one connection. Plan only until reviewed.
todos:
  - id: flags-stub
    content: "Add --serve (exclusive with -c); ping/shutdown loop on stdin/stdout"
    status: pending
  - id: envelope
    content: "Map StatementResult and EngineError to one JSON envelope per request"
    status: pending
  - id: exec-session
    content: "op exec through Session; allow listed meta; refuse watch/clear/shell/exit"
    status: pending
  - id: loop-polish
    content: "One exec at a time; protocol errors; do not write ~/.dql_history"
    status: pending
  - id: local-smoke
    content: "Optional DynamoDB Local serve test when port 8000 is up"
    status: pending
  - id: docs
    content: "Document --serve in rust-docs and dqlrs --help"
    status: pending
  - id: bind-progress
    content: "Optional v1.1 — loopback --bind and progress events"
    status: pending
isProject: false
---

# dqlrs headless worker

Canonical write-up: [`rust-plans/headless_worker.md`](../../rust-plans/headless_worker.md).

Implement only after that document is reviewed. Do not attach the Jupyter kernel in the first Rust PR.
