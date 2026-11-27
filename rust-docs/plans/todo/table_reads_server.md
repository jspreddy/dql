# Table browser: structured reads, built on the server

**Priority:** High  
**Area:** `dqlrs web` table browser (`dqlrs-web/static/table-query.js`, `web/server.rs`)  
**Status:** Steps 1 to 5 done on stacked branches `table-reads/1-contract` to `table-reads/5-cleanup`. Step 6 is deferred. This is the overview. The step plans below own the work.

## Problem

The table browser keeps a structured read: mode, source, key rows, filters, order, and the Keys
box. On Run it goes text, then statement, then text, then statement:

1. `buildReadStatement` in `table-query.js` writes a DQL string.
2. The string goes to `/api/rows` or `/api/explain` as `dql`.
3. `browser_read_dql` parses it to check it is a SELECT or SCAN of that table.
4. It appends `LIMIT n` as text.
5. The engine parses that string again and runs it.

So JavaScript has to write DQL the Rust parser accepts. Every 0.7.12 table-browser bug was the two
sides disagreeing: quoting reserved words, quoting names that are not identifiers, `''` versus the
lexer's backslash escape, a key name that blocked a Scan, and `size` as an attribute. Values have
the same flaw today: `quoteValue` writes `'o''brien'`, which the lexer reads as two strings.

## Goal

The browser sends its read as JSON. The server validates it and builds the statement. The preview
text, the error message, Run, and Explain all come from that one Rust builder. JavaScript no
longer writes DQL.

## Plans

| # | Plan | Lane | Depends on | Size |
| --- | --- | --- | --- | --- |
| 1 | [`table_reads_1_contract.md`](table_reads_1_contract.md): the request shape, and shared golden cases | first | none | small |
| 2 | [`table_reads_2_builder.md`](table_reads_2_builder.md): the Rust builder and validator | server | 1 | large |
| 3 | [`table_reads_3_endpoints.md`](table_reads_3_endpoints.md): `/api/table-read`, and `read` on `/api/rows` and `/api/explain` | server | 2 | medium |
| 4 | [`table_reads_4_browser.md`](table_reads_4_browser.md): the browser sends `read` and shows the server's preview | browser | 1 (3 to ship) | medium |
| 5 | [`table_reads_5_cleanup.md`](table_reads_5_cleanup.md): drop the `dql` path and the JavaScript builder | join | 3, 4 | small |
| 6 | [`table_reads_6_statement.md`](table_reads_6_statement.md): build the `Statement` directly, with a DQL printer | later | 5 | large, deferred |

## Parallel work

```text
1 contract ──┬── 2 builder ── 3 endpoints ──┐
             └── 4 browser ─────────────────┴── 5 cleanup ── (6 statement, later)
```

- **1 goes first.** It fixes the JSON shape and the golden cases everyone tests against.
- **Lane server (2, then 3)** and **lane browser (4)** can run at the same time. The browser can
  build against the contract and a stub endpoint, and switch to the real one when 3 lands.
- **5 joins them.** Once both lanes are in, the old `dql` path and the JavaScript builder go.
- **6 is deferred.** Steps 2 to 5 have the server write DQL text and run it through the parser, as
  today. That removes the JavaScript side of the round trip, which is where the bugs were. Building
  the `Statement` directly, and printing DQL from it, is cleaner but larger, so it waits.

## Rules for every step

- One step per pull request, stacked in order 1, 2, 3, 4, 5.
- No behavior change in a step except the bugs the contract fixes (values with quotes). The golden
  cases say what each read builds.
- The preview shows exactly what Run runs. Nothing builds a statement on one side and runs a
  different one on the other.
- Follow `dqlrs-web/DESIGN.md` for anything visible. `just web-test` and `just verify` pass at each
  step.
