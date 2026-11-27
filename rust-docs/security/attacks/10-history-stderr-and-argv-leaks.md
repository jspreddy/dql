# 10. Smaller leaks: history, stderr, and process arguments

**Severity:** Low. Each leaks a credential or query text rather than rows, or needs a narrow
moment.

## REPL history in plaintext

The REPL appends every command to `~/.dql/history` (`history.rs:203`), unencrypted. Commands
carry literal values from `WHERE` clauses and `INSERT`s (`WHERE ssn = '123-45-6789'`). A
same-user agent reads the file. That is query text, not result rows, but the values are often
data.

**Fix:** seal the history with the cache key, as the other caches are, or keep it per session in
memory unless the person turns it on.

## The token on stderr and in configs

`dqlrs web` prints the full MCP config, bearer token included, every time it starts. An agent
that starts `dqlrs web` itself (`just web`, a test run) reads it from the output. So does anything
that captures the terminal: a log file, `tmux` scrollback, a CI log. Configs pasted into
`.cursor/mcp.json` or `.mcp.json` in the project put the token in the repository, where any
workspace reader, and git history, can see it. Because the token never rotates (03), every copy
stays valid.

**Fix:** follows from 01 to 03: a per-process browser secret that is never printed in full, and a
separate MCP token that the person can revoke. Warn when the MCP config is written inside a
git work tree.

## The keychain secret in `argv`

On macOS, `keychain_store` runs `security add-generic-password … -w <hex>`
(`cache_seal.rs:549`). While that command runs, the secret is in its arguments, visible to every
user through `ps`. It happens once, when the key is first created.

**Fix:** store the item through the Security framework (a crate such as `security-framework`)
instead of the `security` CLI, so the secret never appears in a command line. The same change
allows the presence check in 03.
