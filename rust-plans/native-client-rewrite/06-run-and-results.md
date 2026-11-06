# Run and results

## Controls

| Control | When | What it runs |
| --- | --- | --- |
| **Run** in the editor head | A file is open and nothing is running | `runTarget` of the buffer. Tooltip **Run selection**, shown after about one second of hover. Height 22px. |
| Gutter play | The frame is on screen | The same range as **Run**. A 22px circle, `#2563eb`, sitting just left of the frame, vertically centered on it. Hidden when the frame is scrolled out of view. |
| **Run file** | A file is open | Every statement span in the buffer, after a flush of the autosave. |
| **Stop** | A run is in progress | Cancels the run. The button replaces **Run** while busy. Tooltip **Stop running query (Esc)**. **Run file** is disabled while busy. The gutter play is disabled while busy. |
| Menu **Run file** / **Run folder** | From the tree | See [04-files.md](04-files.md). |

While a run is in progress the tree shows the blue dot on that file, the gutter shows a spinner on the statement in flight, and the results head shows a progress bar when the total is known. The meta reads `n / total`, or **Running…** when the total is unknown. The iced window prints `Running {path}  {phase} {done}/{total}` on a line under the editor. The page puts the fraction in `#result-meta` and the bar in `#run-progress`. Match the page.

Stop produces an error note that contains **Query stopped**, hides the progress bar, and returns the Run button. A throttled insert that is allowed to finish reports **30 affected** and no error note.

A run that fails (missing table, bad syntax) shows an error note, a `×` on that statement, and re-enables Run. The failing statement scrolls into view. The iced window does not scroll to it. The rewrite does.

Per statement, when the file run finishes: `✓` or `×` in the gutter at that statement. A four-statement file shows four successes.

## Tabs

The results head has three tabs: **Result**, **History**, **Lint**.

| Tab | Contents | Count chip |
| --- | --- | --- |
| Result | The latest run only. One card for a single-statement run. | **0 results**, **1 result**, **N results** |
| History | Every result from this file, oldest first, latest pinned near the top of the scroller with a 6–16px gap. | The same result count |
| Lint | One row per diagnostic: **Error** or **Warning**, **Line N**, the message. Empty copy **No issues**. Clicking a row focuses that line. | **1 issue**, **N issues** |

**Clear results** (trash icon, tooltip **Clear results**) wipes the cards and the on-disk cache for this file. The same action is on the file menu. Clearing a folder removes that folder’s cache files and leaves files outside the folder alone.

Results belong to the file. Switching files shows that file’s Result and History, which may be empty. Reload restores them from the query-result cache.

Run file selects the Result tab. History keeps the earlier statements. A five-statement file that ends in a SELECT shows one card on Result (the last SELECT: `beta`, not `alpha`, in the fixture) and five entries on History. Running a selection again switches back to Result with the new latest, and History grows by one.

The iced window shows a single **Results** stack, capped at 20 cards, and has no tabs. The rewrite has the three tabs.

## Cards

A data card shows:

- A chevron to show or hide the statement. Collapsed by default. Hover raises the chevron. Expanded, the statement is highlighted and the chevron is teal. The control sits at the top of the card, aligned with the title, including on a tall card.
- The table name, bold mono, about 13.5px.
- The schema icon (tooltip **Show result structure**), to the left of the row-count pill.
- **1 row** or **N rows**.
- A capacity chip when the value is greater than zero: `12 RCU`, `1.5 WCU`, matching `/^\d+(\.\d+)? (RCU|WCU)$/`. Zero capacity draws no chip.
- A time pill: `4 ms`, or seconds when the run is long (`/^\d+ ms$|^\d+(\.\d)? s$/`).
- The grid.

Spacing: name, schema button, row pill, capacity, time, in that order, on one line. ANALYZE and SCAN of the same table produce two cards. Their column lists match. An ANALYZE capacity chip is non-zero and matches the SCAN capacity text when the engine reports the same units.

The grid scrolls on both axes inside the card. The header row stays pinned vertically. The peek column stays pinned horizontally. Wheel and drag both reach the last column. Even rows use the stripe wash. Hover uses the teal hover wash.

Each statement’s card is its own card (`data-table` on the page). Only one grid is mounted for the focused card body; other cards keep their headers.

## Row peek

The first column is 28px and holds an eye icon, teal, 14px. It is faint until the row is hovered (opacity 0 to 1). Clicking it opens the row sheet in [09-sheets.md](09-sheets.md). The control remains clickable when the grid is scrolled in either axis. A scrollbar must not take the press.

## Explain

An EXPLAIN result is a plan, not a data grid. One line names the operation (`query`), the table, and the index (`TABLE`). Fields, in order, then any extras: **projection expression**, **key condition expression**, **filter expression**, **attribute names**, **attribute values**, **consistent**, **operation**. Values show the rendered expressions (`#f`, `id = `, `n > `, `"a"`, `"ada"`, `true`).

A checkbox switches to the raw JSON and hides the steps. The choice persists (`dqlrs-web.explain-json` on the page). Reload keeps it.

## Help

`HELP SELECT` renders a help card titled **HELP select**, with keyword coloring and example blocks that include `CREATE TABLE`. There is no statement chevron on a help-only card. `HELP` with no topic is titled **Available commands**, lists `select`, and the blurb **Query items from a table or index**.

Nested-path examples from `HELP SELECT` and `HELP SCAN` include the line `WHERE can compare a.b.c and a[0].b on SELECT and SCAN.` Running those examples returns the documented one-row shapes.

The iced window prints help as pretty JSON. The rewrite renders the help card.

## Describe and intelligent match

`DESCRIBE` and the table form of `ls` / `SHOW TABLES` render a describe card: the table name, **Hash Key**, the range key name and type, and the `CREATE TABLE` statement with syntax color.

When the pattern is not an exact name, the note is **No exact match for "{pattern}"** plus **showing similar names** or **showing related keys**. A zero-row scan still says **No rows** in the body and **0 rows** in the meta.

`SHOW TABLES LIKE` with an exact name returns one row and no similar-names note. `ls {name}* refresh=True` refreshes and the cache hint reads like `Refreshed the cached table list`. After `ALTER TABLE … SET THROUGHPUT (2, 3)`, `SHOW TABLES` shows RCU 2 and WCU 3 for that table.

## Cache files

Query results are sealed files under `.dqlrs-web-cache/` (directory mode `0700`). Each file is `query_result_{stamp}.json`, mode `0600`, first byte `1`, and the body does not contain plaintext secrets. The window writes the same cache the page reads, through the studio, not through HTTP.
