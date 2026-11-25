# A grant reservation sticks after its temp table is dropped

Status: fixed on `fix/release-review-followups`. Found on `v-rust` at `90c492c` while reviewing #312 (`85fe411`).

## What goes wrong

1. An agent requests access. The user runs the query, so the request holds `mcp_rN`.
2. The user clicks **Grant**. The grant waits for the session, because a script in the editor is running.
3. That script drops `mcp_rN`. The request goes back to "not run".
4. `GRANT` then fails, because the table is gone.

From then on the request can never be granted or denied. **Deny** answers "rN is being granted". **Run** works, but the next **Grant** answers "rN is already being granted". The agent waits on `request_access` until the server stops.

A store-level test reproduces it: `grant`, then `note_missing(|_| false)`, then `abort_grant`, then `deny` returns `Err("r1 is being granted")`; after `ran`, `grant` returns `Err("r1 is already being granted")`.

## Why

#312 added `granting` to `Request` in `rust-impl/crates/dql-cli/src/web/access.rs`. `Store::grant` sets it, and `finish_grant` and `abort_grant` clear it. `abort_grant` clears it only while the phase is still `Ran`:

```rust
if matches!(request.phase, Phase::Ran { .. }) {
    request.granting = false;
}
```

`note_missing` moves a `Ran` request back to `Open` when its table is gone, and leaves `granting` set. So a drop between `grant` and `GRANT` leaves a reservation nothing can clear. `can_run` ignores `granting`, so a later Run moves the request back to `Ran` with the flag still on.

A smaller gap in the same area: **Run** in one tab while a **Grant** from another tab is waiting replaces `mcp_rN`. The waiting `GRANT` then grants the new result, which the granting user has not seen classified.

## What to change

`abort_grant` clears `granting` whatever the phase. `note_missing` clears it too. `can_run` refuses while `granting` is set ("rN is being granted"). Add a store test for the drop-then-abort sequence: after `abort_grant`, `deny` succeeds.
