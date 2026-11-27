# Same-User Key Store Retrieval, Disk Cache Decryption, and Plaintext History

## Summary

`dqlrs` encrypts `<workspace>/.dqlrs-cache/` (table definitions, local-table definitions, and up to 50 recent human query-result files per `.dql` script) with AES-256-GCM under a per-user 256-bit key (`cache-key`). However, every platform key store used by `cache_seal.rs` releases `cache-key` (and `api-token`) **without an interactive prompt** to any process running as the same OS user. In addition, REPL command history (`~/.dql/history`) is written in plaintext with default file permissions.

## Violated boundary

- **Intended:** An agent running on the machine cannot read data contained within `dqlrs` or its caches without an explicit grant.
- **Actual:** Any same-user local process can retrieve `cache-key` silently and decrypt every cached query result in `.dqlrs-cache/<conn>/results/`, or read plaintext REPL history in `~/.dql/history`.

## Code paths

1. **Query-result cache contents:** `rust-impl/crates/dql-cli/src/web/query_cache.rs` (`save`) writes sealed JSON files at `<workspace>/.dqlrs-cache/<conn-sha256>/results/<script-sha256>/query_result_<ms>.json` (up to `MAX_FILES_PER_QUERY = 50` per `.dql` file). Each sealed payload contains the statement text, column metadata, and full `items` array from human runs in `dqlrs web`.
2. **Key store backends (`rust-impl/crates/dql-cli/src/cache_seal.rs`):**
   - **Linux without keyring / WSL / headless (`file_store`):** Stores raw hex secrets in `~/.config/dqlrs/cache.key` and `~/.config/dqlrs/api-token` (`0600`, owned by the current OS user).
   - **Linux Secret Service (`SecretServiceStore`):** Stores items via `secret-tool store --label=... service dqlrs kind <kind>` and reads them via `secret-tool lookup service dqlrs kind <kind>`. When the desktop session unlocks the default login keyring, any process in that user session can run `secret-tool lookup` with zero user prompt.
   - **Windows DPAPI (`DpapiStore`):** Encrypts `%LOCALAPPDATA%\dqlrs\cache.key` and `api-token` with `CryptProtectData` (current-user scope, no optional entropy) and decrypts with `CryptUnprotectData` passing `CRYPTPROTECT_UI_FORBIDDEN`. Any process running under the same Windows user account can call `CryptUnprotectData` silently.
   - **macOS Keychain (`MacKeychainStore`):** Writes items to `login.keychain-db` by shelling out to `/usr/bin/security add-generic-password -U -a dqlrs -s dqlrs-cache-key -w <hex>` and reads them via `/usr/bin/security find-generic-password -a dqlrs -s dqlrs-cache-key -w`. Because `/usr/bin/security` creates the item, macOS places `/usr/bin/security` in the item's default access control list; while the login keychain is unlocked, **any** local process running as that user can invoke `/usr/bin/security find-generic-password ... -w` to read both `dqlrs-cache-key` and `dqlrs-api-token` without a password or Touch ID dialog.
3. **Environment variable key-path overrides:** `cache_seal.rs` (`env_override_file`) honors `DQLRS_CACHE_KEY_FILE` and `DQLRS_API_TOKEN_FILE` whenever set in the environment.
4. **Plaintext REPL history:** `rust-impl/crates/dql-cli/src/history.rs` (`resolve_history_dir`, `append_command_to_file`) writes every executed REPL command in cleartext to `~/.dql/history` (or `$DQL_HISTORY_DIR/history`) using `fs::create_dir_all` and `OpenOptions::new().create(true).append(true)`—inheriting the process umask (typically `0755` / `0644`) and applying no encryption.

## Circumvention mechanics

1. **Direct cache decryption:** A local agent reads `cache-key` using the platform's silent lookup (`~/.config/dqlrs/cache.key`, `secret-tool`, `CryptUnprotectData`, or `/usr/bin/security`), strips the `dqlrs-seal-v1\n` header and 12-byte nonce from `<workspace>/.dqlrs-cache/<conn>/results/<hash>/query_result_*.json`, and decrypts the AES-256-GCM ciphertext to obtain all cached rows from prior human runs.
2. **Zero-crypto cache decryption via `dqlrs` itself:** Even without implementing AES-256-GCM, an agent with subprocess execution can launch `dqlrs web --dir <workspace> --listen 127.0.0.1:0` (or use the shared `api-token` against the already-running `dqlrs web` server) and call `GET /api/query-results?path=<script.dql>` so `dqlrs` unseals the cache and returns plaintext JSON.
3. **Mining `~/.dql/history`:** Reading `~/.dql/history` reveals partition/sort keys, filter values, emails, identifiers, and literal item payloads from human `SELECT`, `INSERT`, and `UPDATE` commands run in the CLI REPL. (On multi-user Unix hosts with a permissive umask, `0644` also exposes `~/.dql/history` to other OS users.)

## Remediation

- **Stop persisting decrypted-by-default query rows on disk, or gate the key on interactive presence:**
  - Keep web query-result history in process memory by default (cleared when `dqlrs web` exits), or make disk result caching opt-in.
  - If disk result caching is retained, store `.dqlrs-cache` outside the agent-visible workspace and protect the encryption key behind a separate OS service user or hardware/biometric user-presence ACL (for example macOS `SecAccessControlCreateWithFlags` with `.userPresence` via native Security.framework rather than `/usr/bin/security`, avoiding ACL inheritance by arbitrary CLI callers).
- **Seal and `0600`-restrict `~/.dql/history`:** Move REPL history into sealed `0600` storage (`cache_seal::write_sealed`) or enforce `0700`/`0600` modes and encrypt entries at rest.
- **Scope test-only env overrides:** Restrict `DQLRS_CACHE_KEY_FILE` and `DQLRS_API_TOKEN_FILE` to test builds or require explicit opt-in so environment injection cannot redirect key/token lookups.
