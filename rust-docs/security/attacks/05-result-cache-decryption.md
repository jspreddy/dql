# 05. Cached query results, decrypted

**Severity:** High. **Found by:** both; the connection-folder limit by Cursor. **Needs:** a shell
as the person's OS user. **Under `aws-vault exec`:** unchanged through the key or the running
server; the second-server route is limited to memory and DynamoDB Local connections.

## What happens

`dqlrs web` stores each run (statement, columns, and every row) in
`<workspace>/.dqlrs-cache/<conn-sha256>/results/<script-sha256>/query_result_<ms>.json`, up to
50 per `.dql` file, sealed with AES-256-GCM, mode `0600`. Without the key the files are opaque.
A same-user agent has three ways in:

1. **Decrypt with the cache key** (both): read the key (03), strip the `dqlrs-seal-v1` header and
   the 12-byte nonce, decrypt. No AWS credentials are needed.
2. **Ask the running server** (both): `GET /api/query-results?path=<file>.dql` with a token from
   01, 02, or 03.
3. **Start a second server** (Claude): `dqlrs web --dir <workspace>` loads the same key and serves
   the history. This opens the person's folder only when it computes the same connection
   identity: `memory`, or the same DynamoDB Local endpoint. For an AWS connection, a server
   without credentials computes `"unknown"` and opens a different folder (Cursor).

These are rows the person saw, not rows they saved, so they are in scope.

## Reconciled

Claude reproduced (3) on the memory backend. Cursor pointed out that under `aws-vault`, an AWS
connection's folder is keyed by the STS identity, which a server without credentials cannot
compute. Both are right; (3) is narrower than first written, and (1) covers the AWS case.

## Reproduced

Claude: a stored result with a planted value had no plaintext match on disk; a second
`dqlrs web` on the same folder returned the value from `/api/query-results`.

## Code

- `web/query_cache.rs` `save`, `MAX_FILES_PER_QUERY = 50`.
- `session.rs:254` `session_identity`, `attach_connection_storage`.
- `cache_seal.rs`: one key per user.

## Fix

- Keep result history in memory by default; make disk caching opt-in (both).
- Gate the key on presence, or keep it from agents (03).
- If disk caching stays, keep it outside the workspace (Cursor).
