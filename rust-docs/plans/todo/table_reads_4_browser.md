# Table reads 4: the browser sends its read

**Priority:** High  
**Area:** `dqlrs-web/static/table-query.js`, `dqlrs-web/static/app.js`  
**Status:** Done on `table-reads/4-browser`. Step 4 of [`table_reads_server.md`](table_reads_server.md). Needs step 1; ships with step 3.

## Goal

The table browser sends its read as JSON and shows the preview and the error the server builds.

## Change

- `table-query.js` gives `onChange` and `onRun` the read request (step 1), not DQL.
- The DQL preview and the error line ask `POST /api/table-read`, debounced and latest-wins, so
  typing stays smooth and an older answer never replaces a newer one. Run stays disabled while the
  answer is pending or has an error.
- Run sends `POST /api/rows` with the read and the page. Explain sends `POST /api/explain` with
  the read.
- The rows cache keys pages on the `dql` the server returns, which is the canonical text.
- The Keys box highlighting (`keyHighlightSpans`) stays in the browser. It only colors text.

## Tests

- Playwright: the existing table browser tests pass unchanged where the statement is the same.
- New: a filter value with a `'` reads its row; the preview updates as the user types; a stale
  preview answer never shows; an error from the server blocks Run and is shown.

## Out of scope

Deleting the JavaScript builder (step 5).
