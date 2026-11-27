# Table reads 5: remove the old path

**Priority:** High  
**Area:** `dqlrs-web/static/table-query.js`, `web/server.rs`, tests  
**Status:** Done on `table-reads/5-cleanup`. Step 5 of [`table_reads_server.md`](table_reads_server.md). Needs steps 3 and 4.

## Change

- Remove the `dql=` forms of `/api/rows` and `/api/explain`, and `browser_read_dql`'s text checks
  that only the browser used.
- Remove `buildReadStatement`, `readError`, `quoteIdent`, `quoteValue`, the Keys parser, and the
  rest of the JavaScript builder. Keep what the UI still needs: the op lists for the menus, source
  lists, attribute suggestions, and the Keys box highlighting.
- Move the builder cases still in `dqlrs-web/tests/test_table_query.mjs` into the golden file, or
  delete them where a golden case covers them.
- Update `rust-docs/` where it describes the table browser.

## Tests

`just verify`. No test may still call a removed function.
