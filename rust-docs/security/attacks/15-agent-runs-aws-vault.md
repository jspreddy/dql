# 15. The agent runs `aws-vault` itself

**Severity:** Critical when `aws-vault` answers without a person; blocked when every use needs one.
**Found by:** both. **Needs:** a shell as the person's OS user. **Under `aws-vault exec`:** this is
the `aws-vault` side of the same setup.

## What happens

`aws-vault` is on the agent's `PATH`. `aws-vault exec <profile> -- aws dynamodb scan …`, or
`aws-vault export <profile>`, works for the agent whenever `aws-vault` does not need the person:

| Setup | Agent gets credentials? |
| --- | --- |
| macOS keychain backend, unlocked (it stays unlocked for its lock timeout, 5 minutes by default), with a cached STS session | Yes, silently. The person just unlocked it to start `dqlrs` |
| Every exec needs an MFA code, with session caching off or short | No, unless the agent can read the code |
| Linux Secret Service or Windows Credential Manager, unlocked for the login session | Yes |
| `file` backend with `AWS_VAULT_FILE_PASSPHRASE` readable | Yes |
| `pass` backend with a cached `gpg-agent` key | Yes, until the cache expires |
| `credential_process = aws-vault export …` in `~/.aws/config` | Any SDK, including an agent-started `dqlrs`, calls `aws-vault`; same rows as above |
| `aws-vault exec --ec2-server` | Yes. Served on `169.254.169.254` to every local process |

Other credential sources count too: static keys in `~/.aws/credentials`, SSO tokens in
`~/.aws/sso/cache/`, and an agent started from inside `aws-vault exec`, which has the credentials
from birth.

## Status

From `aws-vault`'s documented behavior (both). Not run; it depends on the person's backend.

## Fix

Settings for the person; `dqlrs` can check some and warn.

- MFA on every profile `dqlrs` uses, short sessions, or no session caching (both).
- A keychain that locks quickly, and `--prompt=osascript` so prompts are dialogs on the person's
  screen (both).
- No `AWS_VAULT_FILE_PASSPHRASE`, no `--ec2-server`, no static keys in `~/.aws/credentials`
  (both).
- Do not start an agent, an editor that runs agents, or a terminal from inside `aws-vault exec`
  (Claude).

The check that settles it: from the agent's shell, run
`aws-vault exec <profile> -- aws sts get-caller-identity`. If it prints an identity without the
person typing anything, the agent can read the data.
