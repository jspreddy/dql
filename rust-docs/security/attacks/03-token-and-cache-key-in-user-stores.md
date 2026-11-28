# 03. The token and the cache key are readable by any process of the user

**Severity:** Critical. **Found by:** both; key-path overrides by Cursor. **Needs:** a shell, or
any file or command tool, as the person's OS user. **Under `aws-vault exec`:** unchanged.
`aws-vault` guards AWS keys, not these two secrets; they stay in the default login store.

## What happens

The API token and the cache key are 32-byte secrets per OS user (`cache_seal.rs`). Every store
keeps them from other users and none from the same user:

| Platform | Store | Same-user read |
| --- | --- | --- |
| Linux without a keyring, WSL | `~/.config/dqlrs/api-token`, `cache.key`, `0600` | Read the file |
| Linux with Secret Service | `secret-tool` items | `secret-tool lookup service dqlrs kind <kind>` while the login keyring is unlocked |
| macOS | Login keychain, written by `/usr/bin/security add-generic-password` with no `-T` | `security find-generic-password -s dqlrs-api-token -w`; the item's access list trusts `/usr/bin/security`, so no prompt |
| Windows | DPAPI, current-user scope, no extra entropy, `CRYPTPROTECT_UI_FORBIDDEN` | `CryptUnprotectData` from any process of that user |

- The token is stable (both): every process of the user loads the same one and it never
  rotates, so any copy (store, terminal, log, MCP config) keeps working.
- `DQLRS_CACHE_KEY_FILE` and `DQLRS_API_TOKEN_FILE` redirect key lookups whenever set (Cursor).
  Anything that can set the environment of a `dqlrs` the person starts can point it at a key
  the attacker chose.

## Reproduced

Claude: on Linux without a keyring, `~/.config/dqlrs/api-token` held the running server's token
and authorized `/api/run`. The macOS, Secret Service, and DPAPI rows are from the code (both).

## Code

- `cache_seal.rs:103` `load_api_token`, `:167` `key_file_from`, `env_override_file`.
- `cache_seal.rs:549` `keychain_store`: `security add-generic-password -U -s … -a … -w <hex>`.
- `api_token.rs:19` `load`: one token per user, no rotation.

## Fix

- Run agents as another identity, so these stores are out of reach (README).
- Gate the secrets on presence (both): macOS `SecAccessControlCreateWithFlags` with
  `.userPresence` through the Security framework instead of the `security` CLI; Windows Hello;
  polkit on Linux.
- Mint the API token per process and keep it in memory (01).
- Limit `DQLRS_*_FILE` overrides to test builds (Cursor).
