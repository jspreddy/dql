# DynamoDB Local Unauthenticated Endpoint and Unencrypted Workspace Database

## Summary

When a human develops or inspects local tables using `dqlrs dynamo start` or `dqlrs web --init`, `dqlrs` launches DynamoDB Local on `127.0.0.1:8000` with `-sharedDb` and stores its backing SQLite file unencrypted inside `<workspace>/.dynamodb-local/data/`. Any local agent with either workspace read access or loopback network access can read (and modify) every item in every local table without touching `dqlrs` or MCP.

## Violated boundary

- **Intended:** Data managed through `dqlrs` local environments is subject to the same MCP grant boundary as any other connection; agents cannot read ungranted table rows.
- **Actual:** Both the on-disk backing store inside the workspace and the loopback HTTP listener expose all local table data with no authentication or encryption.

## Code paths

1. **Workspace SQLite data path and `-sharedDb`:**
   - `rust-impl/crates/dql-cli/src/web/dynamodb_local.rs` defines `INSTALL_DIR = ".dynamodb-local"` and `PORT = 8000`.
   - `rust-impl/crates/dql-cli/src/dynamo.rs` (`start_java`) launches `java -jar DynamoDBLocal.jar -sharedDb -dbPath <workspace>/.dynamodb-local/data -port <port>`, creating `<workspace>/.dynamodb-local/data/shared-local-instance.db` in plaintext with default umask.
2. **Unauthenticated loopback HTTP endpoint:** DynamoDB Local accepts standard AWS DynamoDB JSON-RPC requests (`DynamoDB_20120810.Scan`, `Query`, `GetItem`, `PutItem`, `DeleteItem`) over HTTP on `127.0.0.1:<port>`. Because `-sharedDb` is enabled, DynamoDB Local ignores the access key ID and region when partitioning storage—any placeholder `Authorization` header accesses the human's exact tables.
3. **Plaintext instance registry:** `rust-impl/crates/dql-cli/src/dynamo.rs` (`write_record`) writes `~/.local/share/dqlrs/dynamodb-local/instances/<port>.json` and `<port>.log` with default file permissions, recording the active `port`, `pid`, `data_dir`, `compose_file`, `container`, and `workspace` for every managed server.
4. **Docker mode exposure:** When started with `--docker`, `dynamo.rs` publishes `127.0.0.1:<port>:8000` on the host and mounts a named Docker volume (`dqlrs-dynamodb-local-<port>` or `dqlrs-dynamodb-data`), accessible both over loopback HTTP and via local `docker` CLI commands if the user belongs to the Docker socket group.
5. **Directory skip-list mismatch:** `rust-impl/crates/dql-cli/src/web/files.rs` lists `".dynamo-local"` in `SKIP_DIRS` while `dynamodb_local.rs` uses `".dynamodb-local"`. Although `dir_node` already skips entries starting with `.`, `files::resolve_inside` does not block dot-prefixed paths on direct file reads.

## Circumvention mechanics

1. **Workspace-only read (no network, no shell):** An agent that can read files inside the workspace opens `<workspace>/.dynamodb-local/data/shared-local-instance.db` directly and reads every DynamoDB item stored by DynamoDB Local.
2. **Loopback-only HTTP (no filesystem access):** An agent with localhost HTTP access sends a standard DynamoDB `Scan` POST request to `http://127.0.0.1:8000` (or any port listed in `instances/<port>.json`) with dummy credentials (`AWS_ACCESS_KEY_ID=local`, `AWS_SECRET_ACCESS_KEY=local`) and reads all rows directly from DynamoDB Local, completely bypassing `dqlrs web` and MCP.

## Remediation

- **Move DynamoDB Local storage outside the workspace:** Store `-dbPath` under a protected directory owned by the privileged `dqlrs` service principal rather than inside `<workspace>/.dynamodb-local/data/`.
- **Isolate the DynamoDB Local network listener:**
  - Run DynamoDB Local inside an isolated network namespace (or behind a Unix-socket-to-TCP reverse proxy owned by `dqlrs`) so arbitrary processes sharing `127.0.0.1` cannot connect to port `8000` directly.
  - Where TCP loopback must be used on a single-OS-user host without namespaces, partition away from `-sharedDb` using a high-entropy secret passed as the AWS Access Key ID held only in `dqlrs` memory (noting that DynamoDB Local keys its SQLite filename by `<accessKeyId>_<region>.db`, so `-dbPath` must still sit in an unreadable directory).
- **Restrict instance metadata permissions:** Create `~/.local/share/dqlrs/dynamodb-local/instances` with `0700` and instance records/logs with `0600`, and align `SKIP_DIRS` in `web/files.rs` with `".dynamodb-local"`.
