# 12. The agent runs `aws-vault` itself

**Severity:** Critical when `aws-vault` hands out credentials without a person; blocked when
every use needs something only the person can give. **Needs:** a shell as the person's OS user.

## What happens

`aws-vault` is on the agent's `PATH` too. Whether
`aws-vault exec <profile> -- aws dynamodb scan --table-name users` works for the agent depends
on how `aws-vault` is set up, not on `dqlrs`:

| Setup | Agent gets credentials? |
| --- | --- |
| macOS keychain backend, keychain unlocked, and a cached STS session | Yes, silently, until the keychain locks or the session expires. The person just unlocked both by running `aws-vault exec` for `dqlrs` |
| Every exec needs an MFA code (`mfa_serial`), with session caching off or short | No, unless the agent can read the code. A prompt in the agent's terminal is one it cannot answer |
| Linux `secret-service` backend with an unlocked login keyring | Yes |
| `file` backend with `AWS_VAULT_FILE_PASSPHRASE` set anywhere the agent can read | Yes |
| `pass` backend with a running `gpg-agent` that has the key cached | Yes, until the cache expires |
| `credential_process = aws-vault export …` in `~/.aws/config` | Any AWS SDK, including a `dqlrs` the agent starts, calls `aws-vault` for it. Same rows as above |
| `aws-vault exec --ec2-server` | Yes. Credentials are served on `169.254.169.254` to any local process, no prompt |
| `aws-vault exec --ecs-server` | Through the child's environment (the endpoint token is in `AWS_CONTAINER_AUTHORIZATION_TOKEN`), so as in 11 |

The same applies to every other credential source the AWS SDK finds: static keys in
`~/.aws/credentials`, SSO tokens in `~/.aws/sso/cache/`, and environment variables in the
agent's own environment. If the agent was started from a shell inside `aws-vault exec`, it has
the credentials from birth.

## Reproduced

Not run: this depends on the person's `aws-vault` backend and profile settings. The table comes
from `aws-vault`'s documented behavior.

## Fix

These are settings for the person, not changes to `dqlrs`. `dqlrs` can check some of them and
warn.

- Require MFA on every profile `dqlrs` uses, and keep sessions short (`--duration`), or turn off
  session caching, so a later `aws-vault exec` by the agent prompts again.
- On macOS, use a keychain that locks immediately (`aws-vault` keeps its own keychain; set it to
  lock after a short idle time), and prefer `--prompt=osascript` so the prompt is a dialog on the
  person's screen.
- Do not set `AWS_VAULT_FILE_PASSPHRASE`, do not use `--ec2-server`, and do not leave static keys
  in `~/.aws/credentials`.
- Do not start an agent, an editor that runs agents, or a terminal from inside `aws-vault exec`.
  Start only `dqlrs`.
- `dqlrs` could warn at startup when it finds static keys in `~/.aws/credentials`, a
  `credential_process`, or when it runs with no `AWS_VAULT` variable.

The check that settles it: from the agent's shell, run
`aws-vault exec <profile> -- aws sts get-caller-identity`. If it prints an identity without the
person typing anything, the agent can read the data.
