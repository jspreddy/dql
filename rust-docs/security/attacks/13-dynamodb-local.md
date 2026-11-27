# 13. DynamoDB Local: no auth, published on all interfaces, plaintext in the workspace

**Severity:** Medium. **Found by:** both; the all-interfaces publish by Claude, the workspace path,
registry, and skip-list details by Cursor. **Needs:** a TCP connection to the port (from this
machine, or from the network with Docker), or read access to the data folder. **Under `aws-vault
exec`:** unchanged; DynamoDB Local does not use AWS credentials.

## What happens

1. **No authentication** (both). DynamoDB Local accepts any key and secret. With `-sharedDb` it
   also ignores them for storage, so dummy credentials reach the person's exact tables:
   `aws dynamodb scan --endpoint-url http://localhost:8000 --table-name users`.
2. **Published on all interfaces with Docker** (Claude). The compose file `dqlrs dynamo start
   --docker` writes, and `dqlrs-web/examples/docker-compose.yml`, map `"<port>:8000"`, which
   Docker binds on `0.0.0.0`.
3. **Plaintext data in the workspace** (both). The Java setup runs
   `-sharedDb -dbPath <workspace>/.dynamodb-local/data`, which holds an unsealed SQLite file
   (`shared-local-instance.db`). An agent with workspace read access opens it directly (Cursor).
4. **Instance registry** (Cursor). `~/.local/share/dqlrs/dynamodb-local/instances/<port>.json`
   and `<port>.log` record port, PID, data folder, and workspace, with default permissions.
5. **Skip-list mismatch** (Cursor). `web/files.rs` `SKIP_DIRS` lists `.dynamo-local`;
   `dynamodb_local.rs` uses `.dynamodb-local`. Dot folders are skipped in the tree anyway, but
   direct reads through `resolve_inside` do not block them (02).

## Reconciled

Cursor's review says Docker mode publishes `127.0.0.1:<port>:8000`. The code writes
`"{port}:8000"` (`dynamo.rs:675` `generated_compose`), and the examples' compose file maps
`"8000:8000"`. Both bind every interface.

## Code

- `dynamo.rs:621`: `-sharedDb -dbPath`; `:675` `generated_compose`; `write_record`.
- `web/dynamodb_local.rs`: `INSTALL_DIR = ".dynamodb-local"`, `PORT = 8000`.
- `web/files.rs` `SKIP_DIRS`.

## Fix

- Publish on loopback only: `"127.0.0.1:{port}:8000"` in both compose files (Claude).
- Move `-dbPath` out of the workspace, or default to `-inMemory` for scratch work (both).
- `0700` on the instances folder and `0600` on its files; fix `SKIP_DIRS` (Cursor).
- Treat local data like remote data: reachable by agents only through grants. That holds only if
  agents cannot reach the port, which brings back the sandbox in the README.
