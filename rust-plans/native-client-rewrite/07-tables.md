# Tables

Tables mode replaces the editor column with a table page and replaces the file tree with the catalog. The header mode control is how you get there. See [02-shell.md](02-shell.md) for the side head: **Tables**, refresh, cache age.

## Catalog

The filter placeholder is **catalog**. Typing filters the list. The page debounces the filter by 250ms. A request caused by typing sends the pattern and does not send `refresh=1`. The refresh button sends `refresh=1` and keeps the current pattern.

Empty copy is **No tables match {pattern}** when a pattern is set, and **No tables** when the catalog itself is empty.

A row shows the table name and its key columns. Keys are icons, not the words HASH and RANGE. The selected row is the selected wash. Choosing a row loads the first page.

Intelligent match, when the pattern misses:

| Case | Label | Title |
| --- | --- | --- |
| Similar names | **Similar names** | `No exact match for "{stem}", so showing similar names.` |
| Related keys | **Related keys** | `No exact match for "{key}", so showing related keys.` |

## Cache age

The chip and the settings row use one function. `elapsed` is whole seconds from `saved_at_ms` to now.

| Elapsed | Label |
| --- | --- |
| No cache | **not cached** in settings. The side chip is hidden until Tables mode has a cache. Title **No table list cached.** |
| `< 5s` | **just now** |
| `< 60s` | `{n}s ago` |
| `< 3600s` | `{n} min ago` (`n` = elapsed / 60) |
| `< 86400s` | `{n} hr ago` |
| `1 day` | **1 day ago** |
| More | `{n} days ago` |

The settings label wraps the phrase in parentheses: `(just now)`. The side chip shows the same phrase. Hovering the chip says the list is cached. While a refresh is in flight the chip is in the refreshing state.

Refresh on first paint does not force `refresh=1`. The refresh button does, with `async=1` on the page. The window calls the same studio refresh. Clear cache is specified in [08-search-and-settings.md](08-search-and-settings.md). After a clear, the note is **Table cache cleared.** and the age returns to just now.

## Page

The meta line reads **50 rows at a time**. The pager reads **Showing 1–50** and then **Showing 51–51** on a 51-row table. Previous is disabled on the first page. Next is disabled on the last page. The status line can echo `{name} · 50 rows per page`.

**No rows** when the page is empty. **Select a table** when none is selected.

The schema button on the title opens the table structure sheet ([09-sheets.md](09-sheets.md)). Tooltip **Show table structure**.

## Grid

Column order: table keys, then index keys, then the other attributes by name. Classes distinguish table, index, both, and plain. The header row is sticky on vertical scroll and does not change which labels are visible. The peek column is sticky on horizontal scroll.

Header tooltips name the key role (**Table hash key**, **Index hash key**, **Index range key**). The eye opens the row sheet for that item. Paging does not reshuffle columns.

Describe-from-this-page and the structure sheet share the keys already loaded. Opening the sheet does not run another query.
