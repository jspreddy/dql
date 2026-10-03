# Shared fixtures

JSON-lines datasets that acceptance tests `LOAD`. Paths are relative to the
suite root (`black-box-tests/`).

Shared datasets:

- [`pk-sk-records/`](pk-sk-records/) — 1000 hash+range rows for SELECT/UPDATE tests
- [`load-users/`](load-users/) — three-user JSON-lines seed for `LOAD`

Do not add pickle (`.p`) files; both CLIs load JSON and CSV (Rust also loads MessagePack).
