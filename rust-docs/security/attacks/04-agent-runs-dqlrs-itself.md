# 04. The agent runs `dqlrs`, or the AWS SDK, itself

**Severity:** Critical. **Needs:** a shell as the person's OS user, and the person's AWS
credentials in the usual places, which is how `dqlrs` itself finds them.

## What happens

The CLI, the REPL, `--serve`, and `dqlrs web` are human surfaces, but nothing in the binary
tells a person from an agent. An agent can start any of them:

- `dqlrs -c "SCAN * FROM users" --json` runs the query and prints the rows.
- `dqlrs --serve` (stdio) takes JSON lines from its parent with no token, by design, and
  runs them.
- `dqlrs web --listen 127.0.0.1:<port> --dir <the person's workspace>` starts a second server
  that the agent owns, prints its token, and serves everything in 01 and 02. It also opens
  the person's caches (05).
- `dqlrs --mcp` starts an MCP server whose session holds the person's AWS credentials. Its tools
  do not return rows, but the process is the agent's to start with other flags instead.

Under all of these sit the AWS credentials (`~/.aws/credentials`, SSO caches, environment
variables) that `dqlrs` uses. An agent can read DynamoDB with the AWS CLI or SDK and never touch
`dqlrs`.

## Under `aws-vault exec`

When the person starts `dqlrs` with `aws-vault exec <profile> -- dqlrs …`, the credentials are only
in that process's environment. A `dqlrs -c`, `dqlrs --serve`, or `dqlrs web` that the agent starts
has none, and its first DynamoDB call fails. So does the agent's own AWS CLI. This attack then
holds only through other routes:

- reading the credentials from the running `dqlrs`, or from its children (11);
- running `aws-vault` itself, or any other credential source the SDK finds (12);
- a backend that needs no credentials: the memory backend, or DynamoDB Local (09).

A second `dqlrs web` without credentials still reads the cached results (05) and still serves
`.dql` files: neither needs AWS.

## Reproduced

`DQL_BACKEND=memory dqlrs -c "…; SCAN * FROM users"` printed the rows. A second
`dqlrs web` on the same workspace started and answered with its own token (see 05).

## Code

- No check for an agent anywhere in `dql-cli`. A search for agent environment markers or a
  terminal check before running a statement finds only display uses of `is_terminal`.
- `serve/` stdio mode skips the token on purpose (`rust-docs/plans/archive/security/local-api.md`).

## Impact

This is the floor under every other fix: while an agent runs as the person, with the person's
AWS credentials, it can read whatever those credentials can read. Detecting agents (an
environment variable, a missing terminal) is not a boundary, because the agent controls its
own environment and can allocate a terminal.

## Fix

Not fixable inside `dqlrs` alone. The model needs:

- **Credential isolation:** agents run where the person's AWS credentials, key store, and
  home directory are not readable, such as a sandbox ([`SANDBOX.md`](../../../SANDBOX.md)) or a
  separate OS user. The only way out of the sandbox to data is the host's `/mcp`.
- **Least-privilege AWS credentials** for anything an agent can reach. If an agent must run
  where the person's credentials are, the honest statement in the docs is that the grant flow is
  a convention there, not a control.
- Inside `dqlrs`: document that `-c`, `--serve`, and `web` are for people, and make `dqlrs web`
  refuse a workspace that another `dqlrs web` already serves, so 05 at least needs the key.
