# 11. AWS credentials in the `dqlrs` process environment

**Severity:** Critical on Linux; likely on macOS. **Needs:** a shell as the person's OS user,
while a `dqlrs` started with `aws-vault exec` is running.

## What happens

`aws-vault exec <profile> -- dqlrs …` puts temporary credentials (`AWS_ACCESS_KEY_ID`,
`AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`) into the environment of `dqlrs` and nothing else.
The agent's own shell has none, which is the point. But a process's environment is readable by
other processes of the same user:

- **Linux:** `/proc/<pid>/environ` is readable by the same user while the process is
  *dumpable*, which is the default. `tr '\0' '\n' < /proc/$(pgrep -x dqlrs)/environ` prints
  the credentials. With them, `aws dynamodb scan` reads any table those credentials can read,
  for the rest of the session's lifetime (one hour by default), with no `dqlrs` in the way.
- **Linux without Yama, or with `ptrace_scope = 0`:** the same user can also attach a debugger
  and read the process memory, which holds the credentials even after the environment is
  cleared.
- **macOS:** `ps -E -p <pid>` (or `sysctl kern.procargs2`) shows the environment of the user's own
  processes. Not tested in this review; check on the macOS version in use.
- **Children inherit it.** `dqlrs` spawns other programs without removing `AWS_*`: JupyterLab
  for `dqlrs notebook` (and every kernel and terminal it starts, see 08), Java or Docker for
  DynamoDB Local, `security` and `secret-tool` for the key store. Each child's environment is
  readable the same way, and a child is dumpable again after `exec`.

## Reproduced

As an ordinary user (not root), `dqlrs web` was started with fake `aws-vault`-style variables.
A second process of the same user, not a child of `dqlrs`, read `AWS_ACCESS_KEY_ID`,
`AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`, and `AWS_VAULT` from `/proc/<pid>/environ`.

The same test against a process that had called `prctl(PR_SET_DUMPABLE, 0)` showed
`/proc/<pid>/environ` owned by root, and the read failed with `Permission denied`.

## Code

- No `prctl`, `PR_SET_DUMPABLE`, or `PT_DENY_ATTACH` anywhere in the workspace.
- `Command::new` in `notebook/`, `dynamo.rs`, `web/dynamodb_local.rs`, `web/init.rs`, and
  `cache_seal.rs` passes the full environment to every child.

## Fix

- **At startup, before anything else:** on Linux call `prctl(PR_SET_DUMPABLE, 0)`. That makes
  `/proc/<pid>/environ` and `/proc/<pid>/mem` root-owned and refuses `ptrace` from the same
  user. On macOS call `ptrace(PT_DENY_ATTACH)` and check whether the environment is still
  visible through `sysctl`.
- **Move the credentials out of the environment:** read `AWS_*` into memory, build the SDK
  credentials provider from them, then remove them from the environment
  (`std::env::remove_var`). Note that on Linux `/proc/<pid>/environ` shows the original block,
  so this alone is not enough; it matters for the children.
- **Scrub children:** `env_remove` every `AWS_*` variable on each `Command` that does not need
  them. For the notebook kernel, pass credentials over a pipe, not the environment.
- **Shorten the window:** `aws-vault exec --duration=15m`. Stolen session credentials still work
  until they expire.
