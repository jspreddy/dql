---
name: Parity 1 Default AWS Connection
overview: "Make the Rust CLI default to a live AWS DynamoDB connection (SDK credential chain) when -H is not set, matching Python. Keep MemoryBackend for unit tests and an explicit offline path."
todos:
  - id: step-1-build-remote-default
    content: "Step 1: RuntimeEngine::build connects SdkBackend when host is None"
    status: pending
  - id: step-2-reconnect-promote
    content: "Step 2: reconnect can promote Memory→Remote and Local↔AWS"
    status: pending
  - id: step-3-local-roundtrip
    content: "Step 3: local / local off / use region round-trip on live sessions"
    status: pending
  - id: step-4-offline-escape
    content: "Step 4: Document or add explicit memory/offline mode for tests and demos"
    status: pending
  - id: step-5-acceptance
    content: "Step 5: Manual + CLI tests for default AWS, use, local off"
    status: pending
isProject: false
---

# Parity 1: Default AWS Connection

## Current state

Without `-H`, Rust always starts an in-memory engine:

```36:41:rust-impl/crates/dql-cli/src/session.rs
        } else {
            Ok(Self::Memory(FragmentEngine::new(
                Engine::new(dql_engine::MemoryBackend::new())
                    .with_allow_select_scan(allow_select_scan),
            )))
        }
```

`RuntimeEngine::reconnect` is a **no-op** on `Memory`, so `use` / `local off` cannot reach live AWS after a memory start. `SdkBackend` already supports real AWS via the default credential chain when `host` is `None` (`rust-impl/crates/dql-engine/src/aws.rs`).

Python always connects on startup; local is opt-in via `host` (`dql/cli.py` `initialize` → `engine.connect`).

## Goal

| Mode | Python | Rust (target) |
| --- | --- | --- |
| `dql` (no `-H`) | Live AWS | Live AWS (`SdkBackend`) |
| `dql -H localhost` | DynamoDB Local | DynamoDB Local (unchanged) |
| Unit / parity tests | N/A | Keep `MemoryBackend` / `InMemoryEngine` |

## Design decisions

- **Default path is `Remote`**, not `Memory`.
- **Do not remove `MemoryBackend`** — engine parity tests and package smoke memory `-c` paths continue to construct it directly.
- **Optional offline escape hatch** — either document that tests use the engine crate API, or add a small CLI flag / env (e.g. `DQL_BACKEND=memory`) so demos without credentials still work. Prefer env over a new public flag unless needed.
- **`local off` must reconnect to AWS** even if the session started on Local.

## Implementation steps (one commit each)

### Step 1 — Default `SdkBackend` when host is absent

**Files**

- [`rust-impl/crates/dql-cli/src/session.rs`](rust-impl/crates/dql-cli/src/session.rs) — `RuntimeEngine::build`
- [`rust-impl/crates/dql-engine/src/aws.rs`](rust-impl/crates/dql-engine/src/aws.rs) — confirm `SdkConfig { host: None, ... }` path

**Work**

- When `host` is `None`, call `SdkBackend::connect(SdkConfig { region, host: None, port: None, access_key: None, secret_key: None })` and return `Remote`.
- Preserve `allow_select_scan` wiring.
- Surface clear connection errors (missing credentials) instead of silently using memory.

**Gate:** `dql -c "ls"` without `-H` hits AWS (or fails with a credential error, not an empty memory catalog).

**Commit:** `fix(cli): default to AWS SDK backend when -H is unset`

---

### Step 2 — Reconnect can change backend kind

**Files**

- [`rust-impl/crates/dql-cli/src/session.rs`](rust-impl/crates/dql-cli/src/session.rs) — `reconnect`
- [`rust-impl/crates/dql-cli/src/meta/connect.rs`](rust-impl/crates/dql-cli/src/meta/connect.rs) — `handle_use`, `handle_local`

**Work**

- Replace Memory no-op: rebuild `RuntimeEngine` from region + optional local endpoint.
- Support transitions: Memory→Remote (if offline mode exists), Local↔AWS, region switch on AWS.
- Clear `cached_descriptions` on every reconnect (already done for Remote).

**Gate:** Starting with `-H localhost`, then `local off`, then `ls` talks to AWS.

**Commit:** `fix(cli): allow reconnect to promote and switch backends`

---

### Step 3 — `use` / `local` parity polish

**Files**

- [`rust-impl/crates/dql-cli/src/meta/connect.rs`](rust-impl/crates/dql-cli/src/meta/connect.rs)
- [`rust-impl/crates/dql-cli/src/session.rs`](rust-impl/crates/dql-cli/src/session.rs) — `region()`, `session_identity()`

**Work**

- `use <region>` updates region and reconnects (AWS or Local).
- `local [host] [port=…]` / kwargs `host=` `port=` set endpoint and reconnect.
- `local off` clears endpoint and reconnects to AWS.
- Prompt / `whoami` reflect real region and STS identity (not `"memory"`).

**Gate:** Interactive `use` + `local` round-trip matches Python behavior.

**Commit:** `fix(cli): align use and local with Python connect semantics`

---

### Step 4 — Offline / memory escape hatch

**Files**

- [`rust-impl/crates/dql-cli/src/args.rs`](rust-impl/crates/dql-cli/src/args.rs) and/or env handling in `session.rs`
- [`rust-plans/migration-from-python.md`](rust-plans/migration-from-python.md)
- [`rust-impl/README.md`](rust-impl/README.md)

**Work**

- Choose one: env `DQL_BACKEND=memory` **or** keep memory only via engine API / tests.
- Update migration notes: default is AWS; Local still via `-H`; memory is for tests/offline.
- Ensure package smoke that expects memory either sets the escape hatch or uses Local.

**Gate:** Docs and smoke tests agree on how to run without AWS credentials.

**Commit:** `docs(cli): document default AWS and offline memory mode`

---

### Step 5 — Acceptance

**Work**

- Manual checklist: `dql` → `ls`; `use`; `local localhost` → `local off`.
- Add a small CLI integration test that mocks or skips when credentials absent, asserting `RuntimeEngine::build(None)` returns `Remote`.
- Confirm existing `InMemoryEngine` parity suite still passes unchanged.

**Commit:** `test(cli): cover default AWS session construction`

## Out of scope

- Changing AWS credential providers beyond the SDK default chain
- CloudWatch / `watch` (Parity 6)
- Windows credential store quirks

## Risk mitigations

| Risk | Mitigation |
| --- | --- |
| CI / smoke without credentials | Explicit memory/Local path for automated gates |
| Surprise AWS calls for users used to memory default | Migration doc + clear startup errors |
| Meta helpers assuming Memory | Audit `with_memory_*` call sites after default flips |
