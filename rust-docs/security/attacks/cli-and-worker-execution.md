# Direct CLI, REPL, Stdio Worker, and Ambient Credential Execution

## Summary

When `dqlrs` is installed on the same machine where a coding agent can execute shell commands or spawn subprocesses, nothing in `dqlrs` distinguishes a human operator invoking the binary from an agent invoking the binary. The CLI, REPL, stdio `--serve` worker, and secondary `dqlrs web` instances execute arbitrary DQL directly using the OS user's ambient AWS credentials or local DynamoDB endpoint.

## Violated boundary

- **Intended:** CLI, notebook, and web are strictly for humans; agents on the machine cannot run queries or bypass MCP grants.
- **Actual:** Any process able to spawn `dqlrs` (or access the same OS environment) has full query and mutation capabilities.

## Code paths

1. **One-shot command execution (`-c` / `--command`):** `rust-impl/crates/dql-cli/src/lib.rs` (`run_with_args`, `run_command_mode`) builds a `Session` and executes any DQL string passed via `-c`, emitting structured JSON when `--json` is supplied.
2. **Interactive REPL (`dqlrs`):** `lib.rs` (`run_interactive`) reads DQL from stdin/pty when no subcommand is given.
3. **Unauthenticated stdio worker (`dqlrs --serve`):** `rust-impl/crates/dql-cli/src/serve/mod.rs` (`run`) passes `None` as the token to `serve::stdio::run_stdio` (`serve/stdio.rs`). Any parent process that spawns `dqlrs --serve` sends `{"id": "1", "op": "exec", "dql": "..."}` over stdin and receives structured item envelopes on stdout with no authentication check.
4. **Standalone stdio MCP (`dqlrs --mcp`):** `lib.rs` (`mcp::run`) starts an unauthenticated stdio MCP server with its own `Session`, reading `.dqlrs-cache` (`tables/` and `local-tables.seal`) and live `DescribeTable` / `ListTables` metadata even when the human has turned MCP off in `dqlrs web` (or is not running `dqlrs web` at all).
5. **Private `dqlrs web` instance:** An agent can spawn its own `dqlrs web --dir <workspace> --listen 127.0.0.1:<ephemeral-port>` process, which connects to the same AWS account or local endpoint and loads the same `.dqlrs-cache` directory.
6. **Ambient credential chain:** `rust-impl/crates/dql-cli/src/session.rs` (`RuntimeEngine::build`) calls `SdkBackend::connect(SdkConfig::aws(region))` (`crates/dql-engine/src/sdk_backend.rs`), which loads the standard AWS SDK credential chain (`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN`, `~/.aws/credentials`, `~/.aws/config`, `~/.aws/sso/cache`, instance/process credentials).
7. **Co-installed Python client (`dql`):** Where `py-impl` (`dql`) or `aws dynamodb` CLI is also on `PATH`, the same ambient credentials allow direct DynamoDB `Query`/`Scan` calls without invoking `dqlrs` at all.

## Circumvention mechanics

A local agent with command execution capability does not need to interact with the human's running `dqlrs web` server or MCP endpoint:

- Spawning `dqlrs -r <region> --json -c "SCAN * FROM <table> LIMIT 100"` (or piping JSON requests into `dqlrs --serve`) queries live AWS DynamoDB or DynamoDB Local directly and returns raw items on stdout.
- Even if `dqlrs` checked parent process names, environment variables such as `CURSOR_AGENT` / `CLAUDECODE`, or TTY attachment, a same-user subprocess can scrub its environment, allocate a pseudo-terminal (`pty`), or talk directly to AWS / DynamoDB Local using the credentials in `~/.aws` or the process environment.

## Remediation

Preventing a local agent with shell execution from running queries requires an **OS-enforced privilege boundary** between the agent process and the credentials/binary that talk to DynamoDB:

1. **Run coding agents in an isolated OS principal or sandbox:**
   - Execute the agent under a dedicated OS user, container, or sandbox profile (Seatbelt / Landlock / bubblewrap / VM) that has:
     - read/write access only to the project workspace (excluding `.dqlrs-cache` and `.dynamodb-local`),
     - **no** access to `~/.aws`, `AWS_*` environment variables, or the user's login keychain, and
     - network/IPC access only to the scoped `/mcp` socket or endpoint.
2. **Or isolate `dqlrs` credentials behind a privileged helper with interactive user presence:**
   - Hold AWS credentials and database connections inside a separate daemon running under a distinct OS UID/GID (not readable or ptraceable by the agent's UID).
   - Require interactive human confirmation (for example macOS LocalAuthentication / Touch ID, Linux polkit, or a browser WebAuthn prompt) before opening a human CLI, REPL, notebook, or web session, while refusing non-interactive `dqlrs -c` / `dqlrs --serve` execution from untrusted sessions.
3. **Remove unauthenticated `dqlrs --serve` where not needed:** Require notebook kernels to authenticate to the human session broker rather than spawning an unauthenticated `dqlrs --serve` pipe directly against ambient credentials.
