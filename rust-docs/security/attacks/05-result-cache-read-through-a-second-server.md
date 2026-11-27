# 05. Cached query results, decrypted by a second server

**Severity:** High. **Needs:** a shell as the person's OS user.

## What happens

The web editor stores each run's result (the statement and its rows) in
`<workspace>/.dqlrs-cache/<connection>/results/…/query_result_<ms>.json`. The files are
AES-256-GCM ciphertext, mode `0600`. An agent that can read the workspace but not the key sees
nothing, as designed.

A same-user agent does not need to break the cipher:

1. **Start its own server** on the person's workspace:
   `dqlrs web --listen 127.0.0.1:18766 --dir <workspace>`. The server loads the same per-user
   cache key and returns the history in plaintext from `GET /api/query-results?path=<file>.dql`.
2. **Or read the key** (03) and decrypt the files directly.
3. **Or use the person's server** with a token from 01, 02, or 03.

These are rows the person saw, not rows they chose to save, so they are in scope.

## Reproduced

The first server stored a result containing a planted value for `notes.dql`. On disk the file
held no plaintext match. A second `dqlrs web` started on the same folder returned the planted
value from `/api/query-results?path=notes.dql`.

## Code

- `web/query_cache.rs`: results sealed with the per-user cache key.
- `cache_seal.rs`: one key per user, loaded by every `dqlrs` process of that user.
- `web/server.rs` `/api/query-results` (GET): returns decrypted results to any caller with that
  server's token.

## Impact

Every result shown in the web editor stays readable by any same-user agent for as long as the
cache keeps it (up to 50 per file).

## Fix

- Follows from 03: a key that needs the person's presence to unlock, or an agent identity
  that cannot load it.
- Keep result history in memory by default, and write it to disk only when the person turns that
  on.
- Refuse to serve a workspace that another live `dqlrs web` already serves, using a lock file
  with the owner's PID. This only raises the bar: the agent can still read the key.
