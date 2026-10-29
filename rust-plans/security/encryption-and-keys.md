# Plan: encryption and keys

**Group:** keys. **Status:** done.

Encrypt both caches from [`disk-caches.md`](disk-caches.md). The query-result files stay in the workspace, so this cipher is what keeps a workspace reader from seeing rows.

## Cipher

AES-256-GCM. Each write uses a new random 96-bit nonce. The file stores a version byte, the nonce, the ciphertext, and the 128-bit tag. Authentication failure is a cache miss: the server ignores that file and does not treat it as a successful query or a saved definition.

Do not derive a key per file. Do not run Argon2 on each read.

## CPU

These files are small. A definition is a few kilobytes. A result is one statement plus one page of rows, and the table browser pages 50 rows. AES-256-GCM on a CPU with AES-NI or the Apple crypto extensions is on the order of a gigabyte per second per core. A multi-megabyte result is about a millisecond of cipher time. Opening the file and parsing JSON costs more.

`--load-tables` encrypts one small file per table after each describe. The DynamoDB calls dominate. The cipher adds milliseconds across a few thousand tables, not seconds.

A password hash, if one is ever added, runs once when the process loads the key. It does not run per file.

## Key

One random 256-bit key per OS user. The process loads it at startup and keeps it in memory. The key file, when a file is required, does not live under the cache directory and does not live inside the workspace.

| System | Store |
| --- | --- |
| Mac | Login keychain. One item for dqlrs. Not iCloud-synced. Readable after the user is logged in. |
| Windows | DPAPI bound to that Windows user. The wrapped blob sits in `%LOCALAPPDATA%\dqlrs\`. No extra prompt. |
| Linux, desktop session | Secret Service (GNOME Keyring or KWallet). |
| Linux, no keyring | `$XDG_CONFIG_HOME/dqlrs/cache.key`, or `~/.config/dqlrs/cache.key`, mode `0600`. |
| WSL | The Linux path above, inside the WSL home. Not on `/mnt/c`. Do not call the Windows credential store from WSL. |

If the platform store is empty, generate the key, store it, and use it. Do not generate a new key on every start once a key exists. A second key would make the existing ciphertext unreadable.

Losing the key means the caches cannot be decrypted. The server then behaves as if the cache were empty and refetches definitions on demand. Query history for that workspace is gone. Document that on first run. Do not upload the key.

## Rust fit

Prefer a small platform module in `dql-cli` over a new crate dependency unless an existing crate already covers Keychain, DPAPI, and Secret Service without pulling a daemon into headless Linux. The headless fallback is a `0600` file either way.

## Done when

- Round-trip tests write a definition and a query result, drop the in-memory key, load the key again, and read the same values.
- The bytes in `.dqlrs-web-cache` do not contain the fixture query string.
- A test with `DQLRS_TABLE_CACHE` set to a temp dir checks file mode `0600`.
- Key lookup is covered for the file fallback. Platform-store tests are skipped when the store is absent, and they run on a machine that has one.
