# 16. Replacing `dqlrs` or a startup file before the person's next run

**Severity:** High. **Found by:** Cursor. **Needs:** a shell with write access to the person's home
folder, and the person running `dqlrs` again. **Under `aws-vault exec`:** this is how an agent gets
fresh credentials handed to its own code.

## What happens

The standard install puts `dqlrs` in `~/.local/bin/dqlrs`, owned and writable by the user.
`aws-vault exec <profile> -- dqlrs …` finds `dqlrs` through `PATH`. A same-user agent can:

- replace or wrap `~/.local/bin/dqlrs`, or put another `dqlrs` earlier on `PATH`;
- change `~/.zshrc` or `~/.bashrc` (aliases, `PATH`, functions named `dqlrs` or `aws-vault`);
- change the notebook kernel's Python files or `DQLRS_BIN` target (12).

On the person's next run, `aws-vault` hands fresh STS credentials to the agent's code, which runs
any query and passes the person a normal-looking `dqlrs`.

## Status

Cursor's review, from the install docs and the code. Not run.

## Fix

- Install `dqlrs` into a root-owned path such as `/usr/local/bin`, and call it by absolute path in
  the `aws-vault exec` command or alias.
- Keep shell startup files and `~/.local` out of the agent's reach: an agent sandbox or a separate
  OS user (README).
