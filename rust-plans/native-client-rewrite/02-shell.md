# Shell

The window is one screen with a header, a side column, a main column, and a status line. Two modes share the side column: **Query** and **Tables**. The header stays put when the mode changes.

Default size is 1280×800. The title is `dqlrs`.

## Header

Height 36px. The bar uses the dark slate palette in both themes (`#1b2838` light, `#0c1117` dark). Foreground is `#e8eef5`.

Left to right, with 10px gaps and 10px horizontal padding:

1. Wordmark **dqlrs** (13px).
2. Mode group, 6px radius: **Query** and **Tables**. The selected mode is teal (`#0f766e`) with white text. Each button is about 26px tall.
3. Search field, filling the remaining width. Placeholder **Search tables, columns, queries**. Height 24px, pill radius. The dropdown is specified in [08-search-and-settings.md](08-search-and-settings.md).
4. Endpoint pill. On DynamoDB Local the label is the host and port (the page shows `localhost:8000`) and `data-kind` is local: a click does nothing. On AWS the label is `{caller} · {region}` and a click opens the environment card: profile, region, a masked access key (`AKIA…MPLE` shape, never the full key), expiry, account, ARN, and user id. Title of that card: **Environment**.
5. Settings button, then the theme button. Settings sits to the left of theme. Both are 26×22 icon buttons. Tooltips **Settings** and **Theme**.

The page also draws presence dots in the header. The native window does not. See [01-objective.md](01-objective.md).

## Query mode

Vertical split: side, then the editor column. Inside the editor column, a horizontal split: editor, then results.

| Region | Content |
| --- | --- |
| Side head | **Files**, then a **+** button (tooltip **New .dql file** / **New file**). Height about 30px. |
| Side body | The file tree. Background is the side token. |
| Editor head | Open file name, lint chip, **Run file**, **Run**, **Stop**. Height about 36px. |
| Editor | Buffer, gutter, statement frame, gutter play button. |
| Results head | **Results**, tabs, count, clear. |
| Results body | Cards. |

The open file name carries a 2px teal underline. Exactly one tree row is the open file: a 2px teal right border, and it is the current item. Switching files moves both marks.

When no file is open the title is **No file open**. Tables mode shows that title because the editor is not the main surface.

## Tables mode

The same vertical split. The side head becomes **Tables**, then the refresh icon, then the cache-age chip, in that order. **+** is hidden. The tree is hidden. The side body is the catalog filter and the table list.

The main column is the table page: title, schema button, row grid, pager. See [07-tables.md](07-tables.md).

Hash `#tables` on the page opens this mode. The window needs an equivalent: starting in Tables mode when the user last left it is optional; a control that selects it is required.

## Status line

A single line under the body, 12px, muted, on the status token. Idle text is **Ready**. Toasts for save, conflict, and run failure sit above the window at the bottom right and also copy a short line into this status. The status line is not the only place an error appears.

## Splits

Both splits drag. The keyboard nudges them: ArrowLeft and ArrowRight on the side handle, ArrowUp and ArrowDown on the results handle. A step changes the size by more than 10px.

| Split | Persist | Default | Reset |
| --- | --- | --- | --- |
| Side width | `nav` pixels | 220 | Double-click the handle clears the saved width and returns to 220, within 8px |
| Results height | `results` pixels | 46% of the query column | Double-click clears it and returns to that 46%, within 8px |

The iced window starts at 260 and 280. Those are not the defaults. The page CSS is.

While dragging, the saved value stays within 3px of the live size. After reload, the size comes back within 3px.

Limits: a pane does not shrink below 140px. Dragging the results split to the top leaves the editor taller than 140px. The page’s drag limit is an editor head plus 160px of editor, and a results minimum of 120px. Use the stricter of the two so neither the editor nor the results pane collapses.

The side width is one preference shared by Query mode and Tables mode. Changing it in one mode changes the other.

Resize handles are 8px hit targets. The visible grip is 4×42px, color `--grip`, opacity 0.45, and opacity 1 with a 56px width while hovered.

## What is persisted

| Field | Default | Meaning |
| --- | --- | --- |
| `dark` | false | Theme |
| `nav` | 220 | Side width in px |
| `results` | 46% of the query column, stored once the user drags | Results height in px |
| `minimal_write` | false | Write highlight is a gutter bar |
| `even_odd` | true | Alternating read bands |

`write_bands` may appear in old JSON. The window ignores it. The two highlight booleans are the settings.

Not persisted: the open file, the search text, which result cards are expanded, which table is selected, scroll positions.
