# 17. Smaller leaks: REPL history and the keychain secret in `argv`

**Severity:** Low. **Found by:** both; `argv` by Claude, file modes by Cursor. **Under `aws-vault
exec`:** unchanged.

The token on terminal streams, which earlier drafts listed here, is in 01.

## REPL history in plaintext

**Found by:** both. The REPL appends every command to `~/.dql/history` (or
`$DQL_HISTORY_DIR/history`), unencrypted (`history.rs:203`). It creates the folder and file with
the process umask, typically `0755` and `0644`, so on a shared machine other users can read it too
(Cursor). Commands carry literal values from `WHERE` clauses and `INSERT`s: keys, emails, IDs.

**Fix:** seal the history with the cache key and create it `0700`/`0600`, or keep it in memory
unless the person turns it on.

## The keychain secret in `argv`

**Found by:** Claude. On macOS, `keychain_store` runs `security add-generic-password … -w <hex>`
(`cache_seal.rs:549`). While it runs, the secret is in its arguments, visible to every user through
`ps`. It happens once, when the key is created.

**Fix:** write the item through the Security framework instead of the `security` CLI. The same
change allows the presence check in 03.
