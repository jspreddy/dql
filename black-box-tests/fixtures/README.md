# Shared fixtures

JSON-lines datasets that acceptance cases can `LOAD`. Prefer a path relative to
the case directory, or copy a `seed.json` into the case so it stays self-contained.

Do not add pickle (`.p`) files; both CLIs load JSON and CSV (Rust also loads MessagePack).
