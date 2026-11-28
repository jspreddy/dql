# 07. Statement smuggling through the statement splitter

**Severity:** High. **Found by:** Cursor. **Needs:** only the MCP file tools, and the person
running the file. **Under `aws-vault exec`:** unchanged.

## What happens

The web runner splits a script with `split_statements` (`web/shape.rs`) and runs each piece. The
splitter and the parser disagree about strings that end in an escaped backslash:

- `split_statements` ends a quote only when the previous character is not `\`:
  `if ch == open && chars.get(i.wrapping_sub(1)).copied() != Some('\\')`. For `'\\'` the previous
  character is `\`, so it thinks the quote is still open and keeps going past semicolons.
- `dql-parser` (`read_quoted`) reads `\\` as one backslash and closes the string at the next `'`.

So a line that the splitter treats as one statement can hold several. `Engine::execute` runs every
parsed statement in the piece and returns only the last result. An agent writes:

1. `CREATE TEMPTABLE … AS (SCAN * FROM … WHERE attr != '\\')`
2. `GRANT MCP READ ON TEMPTABLE …`
3. a harmless `SELECT … WHERE attr != '\''` whose rows the person sees.

The person sees one result card with the harmless rows. `changed_tables` cannot parse the
multi-statement piece and returns nothing, so the table list does not refresh. `execute_watched`
still sees the grant and notifies the agent.

## Code

- `web/shape.rs` `split_statements` (the check above).
- `dql-parser/src/lib.rs` `read_quoted`.
- `dql-engine/src/engine.rs` `Engine::execute`: loops over all statements, returns `last`.
- `web/server.rs` `run_script`, `changed_tables`, `execute_watched`.

## Status

Cursor's review, from the code. The splitter check was confirmed by reading `web/shape.rs`; the
full chain was not run in this reconciliation.

## Fix

- Split with `dql-parser` spans instead of a second quote scanner.
- Run at most one parsed statement per step (`parse_statement`), and refuse a piece that parses to
  more than one.
- Remove grants from scripts (06).
