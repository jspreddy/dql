# 03. The token and the cache key are readable by any process of the user

**Severity:** Critical. **Needs:** a shell, or any file or command tool, running as the person's
OS user. Every coding agent has one.

## What happens

The API token and the cache key are 32-byte secrets stored per OS user
(`cache_seal.rs`). Each store keeps them from other users, but none keeps them from the same
user:

| Platform | Store | How a same-user process reads it |
| --- | --- | --- |
| Linux without a keyring, WSL | `~/.config/dqlrs/api-token`, `~/.config/dqlrs/cache.key`, mode `0600` | Read the file |
| Linux with Secret Service | `secret-tool` items | `secret-tool lookup …` while the keyring is unlocked, which it is for the whole login |
| macOS | Login keychain, written by `security add-generic-password` with no `-T` list | `security find-generic-password -s dqlrs-api-token -w`. The item's access list trusts `/usr/bin/security`, so the read does not prompt |
| Windows | DPAPI | `CryptUnprotectData` from any process of that user |

The token is also stable: every `dqlrs` process of the user loads the same one, and it is never
rotated. A token read once, from a store, a terminal, a log, or an MCP config, keeps working
across restarts.

## Reproduced

On Linux without a keyring, `~/.config/dqlrs/api-token` held the same value the running
`dqlrs web` used. Hex-encoded, it authorized `/api/run`. The macOS row comes from the code
(`cache_seal.rs:549`); it was not run on a Mac.

## Code

- `cache_seal.rs:103` `load_api_token`, `:167` `key_file_from`: the file fallback under
  `~/.config/dqlrs/`.
- `cache_seal.rs:549` `keychain_store`: `security add-generic-password -U -s … -a … -w <hex>`
  with no `-T`, so no application allow-list and no prompt on read.
- `api_token.rs:19` `load`: one token per user, reused by every process, no rotation.

## Impact

With the token, an agent has everything in 01 and 02. With the cache key, it decrypts the sealed
caches directly (see 05).

## Fix

The same-user boundary cannot be made with file modes. Options, strongest first:

- **Run agents as another identity** (a sandbox or a separate OS user), so the person's stores
  are out of reach. See the README.
- **Require the person's presence to unlock.** On macOS, store the items with an access control
  that needs user presence (`kSecAccessControlUserPresence`) through the Security framework, not
  the `security` CLI. On Windows, use Windows Hello. On Linux, ask through polkit. A background
  process cannot answer the prompt.
- **Mint the API token per process** and keep it only in memory, so a stored value is never a
  credential. Exchange it as in 01.
- Pass secrets to `security` on stdin rather than in `argv` (see 10).
