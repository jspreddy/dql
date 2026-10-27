# PRD: local cache and API security

**Status:** plan. Do not implement from this folder until the phases below are accepted.

## Problem

`dqlrs --web` writes two plaintext caches and serves them, plus live queries, on an unauthenticated loopback port.

- Table names and definitions are JSON under `~/.cache/dqlrs/tables/<sha256>/` (`DQLRS_TABLE_CACHE` overrides the root). The directory hash hides the account name. The files do not.
- Each executed statement is JSON in `<workspace>/.dqlrs-web-cache/<sha256>/query_result_<ms>.json`: the statement text and the result body. Up to 50 files per query file. `.gitignore` skips the directory. The files are still readable by anyone who can read the workspace.
- `--listen` and `--serve --bind` refuse non-loopback addresses. The default web port is `127.0.0.1:8765`. Other users on the machine can still connect. `/api/*` has no token. `/mcp` rejects a non-loopback `Origin` only when that header is present.

An agent whose workspace is this project can open the result files without calling the server. Another local user can `curl` the API and run queries.

## Who this is for

The person running `dqlrs` on their own machine, including under WSL, with a browser UI and with Cursor or Claude able to see the project directory.

## Requirements

1. Query results stay in `<workspace>/.dqlrs-web-cache/`. Do not relocate that cache.
2. Table definitions stay under the existing cache root (`DQLRS_TABLE_CACHE`, else `$XDG_CACHE_HOME/dqlrs/tables`, else `~/.cache/dqlrs/tables`).
3. Both caches are owner-only on disk: directories `0700`, files `0600`.
4. Both caches are encrypted at rest with AES-256-GCM. A reader without the key sees ciphertext.
5. One 256-bit key per OS user, created once, loaded at process start. It is not stored beside the cache files.
6. Mac, Windows, Linux, and WSL each have a key location defined in [`encryption-and-keys.md`](encryption-and-keys.md).
7. Web `/api/*` and `/mcp` require a per-process token. `--serve` on loopback requires the same kind of token. Stdio `--serve` stays a child process with no TCP token.
8. Listen addresses stay loopback-only.
9. AWS secret keys and session tokens stay out of the UI, the caches, and the API. The header already masks the access key id and does not read the secret.

## Non-goals

- Stopping a process that runs as the same OS user and can read the key store or the server's memory. That process is the user.
- TLS, public bind, or multi-user accounts on one server.
- Encrypting `.dql` source files.
- Moving `.dqlrs-web-cache` out of the workspace.
- A passphrase prompt on every cache read.

## Decisions

| Topic | Decision |
| --- | --- |
| Result location | Stays in the workspace |
| Protection of those files | Encrypt in place, plus `0600` |
| Cipher | AES-256-GCM, a new nonce on every write |
| Key lifetime | Load once per process. Do not run a password hash per file |
| API | Loopback plus a startup token |
| Same-user agent with the key | Accepted as the user. Document it. Do not build a second lock for it |

## Phases

1. **File mode.** Create cache directories `0700` and files `0600`. Leave paths unchanged. Query results stay in the workspace.
2. **Token.** Mint a token when the web server or TCP `--serve` starts. Require it on `/api/*`, `/mcp`, and TCP `--serve`. The page the server serves sends the token.
3. **Encryption.** Encrypt both caches with the platform key. Existing plaintext files are read once, rewritten as ciphertext, and not left beside the new files.

Phase 1 is useful before the key store exists. Phase 3 is what stops a workspace reader who does not have the key.

## Done when

- A second OS user cannot read either cache directory.
- A workspace listing shows `.dqlrs-web-cache` still in the project, and a result file is not valid JSON plaintext.
- `curl` to `/api/run` or `/mcp` without the token fails.
- `curl` with the token still runs a query on loopback.
- Cold start on Mac, Windows, Linux with a keyring, Linux without a keyring, and WSL finds the key in the place that platform's plan names.
