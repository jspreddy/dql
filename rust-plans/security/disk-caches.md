# Plan: disk caches

**Group:** disk. **Status:** done.

The product has two caches. This plan changes how they are written. It does not move the query-result cache.

## Table definitions

`DiskTableCache` in `rust-impl/crates/dql-cli/src/table_cache.rs`.

| Piece | Today |
| --- | --- |
| Root | `DQLRS_TABLE_CACHE`, else `$XDG_CACHE_HOME/dqlrs/tables`, else `~/.cache/dqlrs/tables`, else `.cache/dqlrs/tables` |
| Directory | `<root>/<sha256 of the endpoint key>/` |
| Names | `NameFile`: timestamp and the name list |
| Definition | `DefinitionFile`: timestamp, status, schema string |

`--load-tables` fills a definition for every table. A refresh rewrites entries that were already fetched. Files are plaintext JSON. The process creates them with the default umask.

**Target.** Same paths. The directory is `0700`. Each file is `0600`. After the encryption phase, the bytes on disk are AES-256-GCM ciphertext with a header the server can recognize, not a JSON object.

## Query results

`rust-impl/crates/dql-cli/src/web/query_cache.rs`.

| Piece | Today |
| --- | --- |
| Root | `<workspace>/.dqlrs-web-cache/` |
| Directory | `<sha256 of the workspace-relative path>/` |
| File | `query_result_<unix-ms>.json` |
| Body | `{ query, result, success }` |
| Cap | 50 files per query file. Older files are deleted |

`.gitignore` already ignores `.dqlrs-web-cache/`. The UI reloads this history for the open file.

**Target.** The same directory, the same file names, the same cap, still inside the workspace. Directories `0700`, files `0600`. After encryption, `query` and `result` are inside the ciphertext. A text editor opened on the file does not show the statement or the rows.

Do not add a second result store under `~/.cache`. Do not keep a plaintext copy next to the ciphertext after migration.

## Migration

On startup, if a cache file is plaintext JSON from this version, read it, write the ciphertext to the same path, and remove the plaintext. A crash mid-migration may leave one file half-written. Write to a temporary file in the same directory, fsync, then rename over the old path.

`DQLRS_TABLE_CACHE` keeps working. Tests that point it at a temp directory keep doing that. The mode and the ciphertext apply there too.

## Out of scope

- Moving `.dqlrs-web-cache`.
- Encrypting `.dql` files or the REPL history file.
- Sharing one cache directory between OS users.

## Done when

- New table-cache and result-cache files are mode `0600`, and their parent directories are `0700`.
- The result directory is still `<workspace>/.dqlrs-web-cache/`.
- A result file written after the encryption phase does not contain the query string or a row value as plaintext.
- Existing tests that read result JSON through the server still see the decrypted statement and rows.
