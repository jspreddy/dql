# Structure and row sheets

Both sheets are full-window dialogs. The card fills the viewport, square corners, scrim behind it. Escape closes the sheet. The close icon is the X at the top right. A click on the scrim outside the card closes it. While a sheet is open, the gutter play button is not visible and does not receive clicks. The sheet itself paints above every float.

Opening a sheet does not run a query. The rows already on screen are the source.

## Result structure

Opened from the schema icon on a result card. Tooltip **Show result structure**.

| Part | Content |
| --- | --- |
| Kicker | **Result structure**, drawn uppercase, 11px, bold, teal ink, tracking `0.08em`. |
| Info | The info icon. Tooltip and accessible name: **Based on this query result, not the entire table.** |
| Title | The result title, usually the table name. Fallback **Result**. |
| Key chips | Hash and range columns with the hash and range icons. Prefer columns tagged as table keys. Otherwise use the snapshot keys. |
| Query | The statement, syntax-colored, in an 8px block. |
| Summary | `{n} column` or `columns`, ` · `, `{m} row` or `rows`. Example **3 columns · 3 rows**. |
| Grid | Headers **Column**, **Type**, **Role**, **Present**, **Sample**. Header labels are drawn uppercase. |
| Footer | **Types come from this result. No extra query was run.** |

The Playwright assertion on the foot is the sentence **No extra query was run.** The full footer above is what the page and the iced window both render.

### Grid cells

- **Type.** One chip per distinct kind in the loaded values: **STRING**, **NUMBER**, **BOOL**, **NULL**, **LIST**, **MAP**. No values but a present count yields NULL. Nothing at all yields an empty chip labeled **—**.
- **Role.** **Hash key**, **Range key**, **Index hash**, **Index range**, with the matching icon. Several roles join. No role is a muted **—**.
- **Present.** A bar at `round(present / total * 100)%`, class sparse when present is below total, label `present/total` (for example `3/3`, `2/3`).
- **Sample.** One formatted value. The tooltip repeats it.

## Table structure

Opened from the schema icon on the table page. Tooltip **Show table structure**.

Same sheet, with these differences:

| Part | Content |
| --- | --- |
| Kicker | **Table structure** |
| Info | Hidden. |
| Title | The table name. Fallback **Table**. |
| Query | Hidden. |
| Summary | The same column and row counts, plus ** on this page**. Example **3 columns · 3 rows on this page**. |
| Footer | **Types come from this page and the keys already listed. No extra query was run.** |

Roles still name hash and range. The page’s table-structure test expects **Hash key**, **Range key**, and **—**.

## Row

Opened from the eye on a result row or a table row. Tooltip **Show row JSON**.

| Part | Content |
| --- | --- |
| Kicker | **Row**, uppercase, same style as the structure kicker. |
| Title | From `rowPopupTitleColumn`. |
| Chips | The other key columns, under the title. |
| Body | Pretty JSON, 12.5px mono, syntax-colored (keys in the keyword color, strings, numbers). |

### Title

1. Consider the table hash column first, then the other columns.
2. The first column whose value is a non-empty string of length 48 or less wins.
3. The title is `{name} = {value}`. If none qualifies, the title is **Row**.

The fixture row titles **id = a**. Chips on that result are **sk = 2** (range) and **n = 7** (hash), because the title consumed `id`. A table-page peek of the same row shows the title **id = a** and one chip, **sk = 2**.

Chip values that are longer than 48 characters truncate with `…`. Null is the text `null`. Roles are `both`, `hash`, `range`, and `index`, and the chip color follows the role.

### JSON order

Keys follow the column list, skipping keys the row does not have. Keys on the row that are not in the column list follow, sorted with localeCompare. The document is `JSON.stringify` with an indent of 2.

The iced window writes that order by hand because `serde_json::Map` sorts keys. The rewrite keeps column order. A row whose columns are `id`, `sku`, `name` prints `id`, then `sku`, then `name`.

`table_name` for the structure title strips a `--` comment from each line before it looks for `SELECT` or `SCAN` and `FROM`. A leading comment does not hide the table. `ANALYZE` is skipped when it is only a prefix. The query block shows the original statement, comments included.
