# 04. The agent runs `dqlrs`, or the AWS SDK, itself

**Severity:** Critical without `aws-vault`; narrowed with it. **Found by:** both. **Needs:** a
shell as the person's OS user. **Under `aws-vault exec`:** the agent's own `dqlrs` and AWS CLI
have no credentials. The attack then holds only through 13, 14, 15, and 16.

## What happens

Nothing in the binary tells a person from an agent. An agent can start any human surface:

- `dqlrs -c "SCAN * FROM users" --json`, or the REPL.
- `dqlrs --serve` on stdio, which takes JSON lines from its parent with no token, by design.
- `dqlrs web --listen 127.0.0.1:<port> --dir <workspace>`: a second server the agent owns.
- `dqlrs --mcp`, Python `dql`, or the AWS CLI and SDK directly.

All of them use the AWS credentials the SDK finds: `~/.aws/credentials`, SSO caches,
environment variables. An environment-variable or terminal check is no boundary: the agent
controls its environment and can allocate a terminal.

## Under `aws-vault exec`

With credentials vended only to the person's `dqlrs`, every surface above fails against live AWS
(`SdkBackend::connect` finds nothing). What remains:

- reading credentials from the running `dqlrs` or its children (14), or from the notebook (12);
- running `aws-vault` itself, or another credential source the SDK finds (15);
- replacing the `dqlrs` binary before the person's next run (16);
- backends that need no credentials: memory, and DynamoDB Local (13).

A second server without credentials computes the connection identity `"unknown"`
(`session.rs:254`), so it does not open the person's AWS cache folder (Cursor). It still opens
memory and DynamoDB Local caches, and the cache key still decrypts everything (05).

## Reproduced

Claude: `DQL_BACKEND=memory dqlrs -c "…; SCAN * FROM users"` printed the rows, and a second
`dqlrs web` on the same workspace answered with its own token. The `aws-vault` behavior is from the
code (both).

## Fix

Not fixable inside `dqlrs` alone. It needs credential isolation (a sandbox or a separate OS user
for agents; see [`SANDBOX.md`](../../../SANDBOX.md)) or least-privilege credentials for anything an
agent can reach. Inside `dqlrs`: document that `-c`, `--serve`, and `web` are for people.
