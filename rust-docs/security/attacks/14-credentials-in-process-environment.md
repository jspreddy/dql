# 14. AWS credentials in the `dqlrs` process environment

**Severity:** Critical on Linux. **Found by:** both; reproduced by Claude. **Needs:** a shell as the
person's OS user, while an `aws-vault exec`'d `dqlrs` runs. **Under `aws-vault exec`:** this is
where the credentials are.

## What happens

`aws-vault exec <profile> -- dqlrs …` puts `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and
`AWS_SESSION_TOKEN` into the `dqlrs` environment (`web/environment.rs` reads `AWS_VAULT` and the
key id from it).

- **Linux** (both): `/proc/<pid>/environ` is readable by the same user while the process is
  dumpable, which is the default. The credentials work with `aws` for the session's lifetime.
- **Debugger attach** (Claude): where Yama allows it (`ptrace_scope = 0`, or no Yama), the same
  user can also read process memory.
- **macOS** (open): see below.
- **Children inherit it** (both): `dqlrs` spawns JupyterLab, Java or Docker, `security`, and
  `secret-tool` without removing `AWS_*`. Each child is dumpable again after `exec`.
- **`--ecs-server`** (Cursor): the endpoint's `AWS_CONTAINER_AUTHORIZATION_TOKEN` is in the same
  environment, so it leaks the same way.

## Reconciled

- **Yama.** Cursor's review says the `/proc/<pid>/environ` read needs `ptrace_scope = 0`.
  Claude's test ran on a kernel without Yama. Reading `/proc/<pid>/environ` is a ptrace *read*
  check; Yama's `ptrace_scope` restricts *attach*. So the read is expected to work on a stock
  Ubuntu with `ptrace_scope = 1`. **To confirm** on a stock Ubuntu: as one user, start a process
  with a variable set, then read its `environ` from an unrelated shell of that user.
- **macOS.** Cursor's review says SIP blocks reading another process's environment; Claude's says
  `ps -E` likely shows it for the user's own processes. Neither was run. **To confirm** with
  `ps -E -p <pid>` against an `aws-vault exec`'d process.

## Reproduced

Claude, as an ordinary (non-root) user: `dqlrs web` started with fake `aws-vault`-style variables.
An unrelated process of the same user read all four from `/proc/<pid>/environ`. A process that had
called `prctl(PR_SET_DUMPABLE, 0)` had a root-owned `environ`, and the same read failed with
`Permission denied`.

## Code

- No `prctl`, `PR_SET_DUMPABLE`, or `PT_DENY_ATTACH` in the workspace.
- `Command::new` in `notebook/`, `dynamo.rs`, `web/dynamodb_local.rs`, `web/init.rs`,
  `cache_seal.rs` passes the whole environment.

## Fix

- At startup, `prctl(PR_SET_DUMPABLE, 0)` on Linux, `ptrace(PT_DENY_ATTACH)` on macOS (Claude).
- Move the credentials into memory and `env_remove` every `AWS_*` on children that do not need
  them; pass the kernel's over a pipe (both).
- Prefer `--ecs-server` over plain variables, never `--ec2-server` (Cursor); shorten
  `--duration` (Claude).
