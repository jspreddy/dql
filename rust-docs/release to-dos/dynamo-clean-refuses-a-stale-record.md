# Dynamo clean can never remove a record whose pid was reused

Status: fixed on `fix/release-review-followups`. Found on `v-rust` at `90c492c` while reviewing #313 (`792584f`).

## What goes wrong

After #313, `dqlrs dynamo stop` refuses a recorded pid that is no longer DynamoDB Local, and says "the recorded process is gone". The record stays.

- `dqlrs dynamo clean` on that port fails with the same error, every time. Its data folder and its record are never removed.
- While the reused pid lives and the port is open, `dqlrs dynamo` keeps listing the record as running.
- `dqlrs dynamo stop` on several ports stops at the first such record. The servers after it keep running.

## Why

`clean` in `rust-impl/crates/dql-cli/src/dynamo.rs` starts with `stop(server)?`. `stop` now returns an error for a pid whose command line is not the DynamoDB Local jar, so `clean` returns before `remove_dir_all` and `remove_record`. The `stop` command loops over its targets with `stop(&server)?`, so the first error ends the loop.

## What to change

When the recorded pid is not DynamoDB Local, treat the record as stale rather than as a failure. `stop` reports that the recorded process is gone and forgets the record without killing anything. `clean` continues to remove the data folder and the record. The `stop` loop reports a stale record and moves on to the next port. Extend the `sleep` test from #310: `clean` on that record succeeds and leaves the `sleep` process alive.
