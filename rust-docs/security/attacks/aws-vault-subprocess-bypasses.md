# Subprocess Bypasses Under `aws-vault exec <profile> -- dqlrs`

## Summary

Vending AWS credentials exclusively through `aws-vault exec <profile> -- dqlrs ...` eliminates ambient `~/.aws/credentials` files and keeps `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and `AWS_SESSION_TOKEN` out of the coding agent's own process environment. A bare `dqlrs -c`, `dqlrs --serve`, or `aws dynamodb` subprocess spawned directly by the agent therefore has **no AWS credentials** and cannot query live AWS on its own.

However, **an agent able to spawn local subprocesses as the same OS user can still obtain ungranted AWS and local data** through six remaining paths: riding the already-running `aws-vault`-credentialed `dqlrs` servers, reusing an unlocked `aws-vault` backend or cached STS session, reading vended credentials from the live child environment where OS settings permit, decrypting `.dqlrs-cache` via the separate OS login keystore, shadowing the user-writable `dqlrs` binary before the next human run, or querying DynamoDB Local directly.

## What `aws-vault exec` successfully prunes

- **Direct uncredentialed AWS CLI/worker invocations:** Spawning `dqlrs -c`, the `dqlrs` REPL, `dqlrs --serve` (stdio), `dqlrs --mcp` (stdio), a fresh `dqlrs web`, Python `dql`, or `aws dynamodb` from the agent's clean environment fails against live AWS because `SdkBackend::connect` finds no ambient credentials.
- **Uncredentialed `dqlrs web` cache-folder matching on AWS:** `Session::attach_connection_storage` (`rust-impl/crates/dql-cli/src/session.rs`) hashes `session_identity()` (the STS caller identity) into `<workspace>/.dqlrs-cache/<conn-sha256>/`. Without STS credentials, a freshly spawned `dqlrs web` computes `"unknown"` and does not attach to the human's AWS connection cache folder.

## Remaining subprocess circumvention paths

### 1. Delegating queries to the human's running `dqlrs` processes

When the human runs `aws-vault exec <profile> -- dqlrs web`, `dqlrs --serve --bind`, or `dqlrs notebook`, **that running process already holds the vended STS credentials**. An agent subprocess (`curl`, `python3`, etc.) needs zero AWS credentials of its own:

- Against `dqlrs web` or `dqlrs --serve --bind`, the subprocess obtains `api-token` from `.cursor/mcp.json`, `GET http://127.0.0.1:8765/`, stderr logs, or the OS login keystore and executes arbitrary DQL via `POST /api/run`, `GET /api/rows`, or TCP `exec` (see [`shared-token-and-api-escalation.md`](shared-token-and-api-escalation.md) and [`loopback-and-terminal-token-exposure.md`](loopback-and-terminal-token-exposure.md)).
- Against `dqlrs notebook`, reading `~/.local/share/dqlrs/notebook/jupyter-runtime/jpserver-*.json` or `kernel-*.json` lets a subprocess run arbitrary DQL in `DqlKernel`—or open a shell via Jupyter's `/api/terminals` **inside** the `aws-vault` process tree and read `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and `AWS_SESSION_TOKEN` directly from that shell's environment (see [`notebook-runtime-and-outputs.md`](notebook-runtime-and-outputs.md)).

### 2. Reusing an unlocked `aws-vault` backend or cached STS session

When a human runs `aws-vault exec <profile> -- dqlrs ...`, `aws-vault` unlocks its backing store and (by default) caches the temporary STS session credentials inside that vault:

- **macOS (`keychain` backend):** `aws-vault` uses a separate keychain (`aws-vault.keychain-db`) that stays unlocked until its lock timeout elapses (5 minutes by default, or longer if configured). While unlocked, any same-user subprocess invoking `aws-vault exec <profile> -- dqlrs --json -c "..."` (or `aws-vault export <profile>`) receives the cached STS session **with no password prompt**.
- **Linux Secret Service / Windows Credential Manager:** Those backends normally stay unlocked for the duration of the desktop login session, allowing `aws-vault exec <profile> -- ...` from an agent subprocess to succeed silently at any time unless per-invocation MFA (`--no-session` + hardware MFA) or `pass`/GPG pinentry confirmation is enforced.

### 3. Extracting vended credentials from the live `dqlrs` process or metadata server

By default (`aws-vault exec` without `--ecs-server`), `aws-vault` injects `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and `AWS_SESSION_TOKEN` into the child `dqlrs` environment (confirmed in `rust-impl/crates/dql-cli/src/web/environment.rs`, where `EnvFacts::from_env` reads `AWS_VAULT`, `AWS_ACCESS_KEY_ID`, and `AWS_CREDENTIAL_EXPIRATION` from `std::env::var`):

- **Linux / WSL with permissive ptrace scope (`kernel.yama.ptrace_scope = 0`):** Any subprocess running under the same UID can read `/proc/<dqlrs-pid>/environ` directly, extract the live STS variables, and pass them to `dqlrs -c`. (On Linux with `yama.ptrace_scope >= 1` and on macOS with SIP, non-ancestor process-environment reads are blocked by the kernel.)
- **`aws-vault exec --ec2-server`:** If `--ec2-server` is used, `aws-vault` serves temporary credentials on `http://169.254.169.254/latest/meta-data/iam/security-credentials/` to **every** local process without a token. (`--ecs-server` avoids global exposure by requiring a per-session `AWS_CONTAINER_AUTHORIZATION_TOKEN` env var, though that token still resides in `dqlrs`'s process environment.)

### 4. Decrypting `.dqlrs-cache` via OS keystore subprocesses

`aws-vault` protects AWS credentials, **not** `dqlrs`'s cache key. Even on macOS—where `aws-vault` uses a locked `aws-vault.keychain-db`—`cache_seal.rs` stores `dqlrs-cache-key` and `dqlrs-api-token` in the **default unlocked `login.keychain-db`** via `/usr/bin/security` (and in unlocked Secret Service, DPAPI, or `~/.config/dqlrs/cache.key` on Linux/Windows). An agent subprocess can invoke `/usr/bin/security` / `secret-tool` / DPAPI to fetch `cache-key` and decrypt every cached result file under `<workspace>/.dqlrs-cache/*/results/` without ever touching `aws-vault` (see [`cache-and-keystore-access.md`](cache-and-keystore-access.md)).

### 5. Shadowing user-writable `dqlrs` binaries or startup files

Standard installation (`rust-docs/README.md`) writes `dqlrs` to `~/.local/bin/dqlrs`, owned and writable by the user. Because `aws-vault exec <profile> -- dqlrs ...` resolves `dqlrs` through `PATH`, a same-user agent subprocess with write access to `$HOME` can replace or wrap `~/.local/bin/dqlrs` (or modify `~/.zshrc` / `~/.bashrc` or `~/.local/share/dqlrs/notebook/src/dql_notebook_kernel/*.py`). On the next human `aws-vault exec` run, `aws-vault` hands fresh STS credentials directly to the modified binary.

### 6. Querying DynamoDB Local directly

`aws-vault` does not guard local development databases. Against `dqlrs dynamo` (`127.0.0.1:8000` with `-sharedDb`), an agent subprocess can still run `AWS_ACCESS_KEY_ID=local AWS_SECRET_ACCESS_KEY=local dqlrs -H localhost -p 8000 --json -c "SCAN * FROM ..."` or read `<workspace>/.dynamodb-local/data/shared-local-instance.db` directly (see [`dynamodb-local-bypass.md`](dynamodb-local-bypass.md)).

## Remediation

To make `aws-vault exec <profile> -- dqlrs` hold against an agent that can spawn subprocesses:

1. **Run the coding agent in a separate OS UID, container, or Seatbelt/bubblewrap sandbox** that cannot:
   - invoke `aws-vault` or access `aws-vault`'s keychain / Secret Service socket,
   - access the user's login keystore (`dqlrs-cache-key`, `dqlrs-api-token`),
   - inspect `/proc/<dqlrs-pid>/environ` or write to `~/.local/bin/dqlrs`, `~/.local/share/dqlrs/`, or shell startup files, or
   - reach arbitrary loopback ports (`8000`, Jupyter ports) other than the scoped `/mcp` endpoint.
2. **Harden `aws-vault` invocation settings even with sandboxing:**
   - Prefer `aws-vault exec --ecs-server` (never `--ec2-server`) so long-lived STS secrets are not held in static environment variables, and scrub `AWS_*` variables before spawning any child process from `dqlrs` (`notebook` terminals, `shell` meta-command).
   - Use short keychain lock timeouts or `--prompt` / hardware-presence backends on `aws-vault` so an unlocked vault window does not linger after startup.
   - Install `dqlrs` into a root-owned read-only path (for example `/usr/local/bin/dqlrs`) rather than user-writable `~/.local/bin/dqlrs`.
