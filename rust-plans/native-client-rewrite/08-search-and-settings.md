# Search and settings

## Title search

The field sits in the header between the mode group and the settings button. See [02-shell.md](02-shell.md).

| Behavior | Rule |
| --- | --- |
| Debounce | 150ms after the last key. |
| Empty or whitespace | Close the list and clear the hits. |
| Groups | **Tables**, then **Columns**, then **Queries**. Column and index hits share the Columns group. Headers render uppercase. |
| Row | A 58px kind label (**Table**, **Column**, **Index**, **Query**) and the title. A second line carries the detail: table name, index name, or path. |
| Empty result | **No matches**. A transport error shows the error text in that slot. |
| Width | The list is exactly as wide as the field. |
| Place | 6px under the field, floating. It does not push the header or the editor. |
| Arrows | Up and Down move the highlight and wrap. |
| Enter | Opens the highlighted row. |
| Escape | Hides the list and keeps the text. |
| Click outside | Dismisses the list. Clicking the field again with text still in it reopens the search when the debounce is idle. |

Opening a hit:

- **Query.** Switch to Query mode, open the file, and scroll the editor so the caret is on the first code character of that statement. The page scrolls to the hit offset. The iced test puts the caret at line 1, column 0 for a hit on the first statement of `read.dql`.
- **Table, column, or index.** Clear the catalog filter, select that table, and switch to Tables mode. The rows title is the table name.

A table that is only in the list, with no cached columns, returns a Table hit and no Column hits. A nonsense column returns **No matches**.

The list paints above the editor and above the gutter play button. Clicks on the list do not fall through to Run.

## Settings

The button is immediately left of the theme button. The card is 232px wide, 8px padding, 10px radius, hairline border, and the dropdown shadow. It opens 6px under the button, right-aligned to the button (`left = max(8, button.right - 232)`). A click on the scrim outside the card closes it. Escape closes it, and does not also stop a query. See [10-interaction.md](10-interaction.md).

Three sections, 11px uppercase labels:

### Highlight

| Control | Default | Effect |
| --- | --- | --- |
| **Minimal write highlight** | off | Write washes become the 3px gutter bar. |
| **Even/odd highlight** | on | Read lines alternate; write lines use the even wash or the even bar. |

Unchecking even/odd removes the alt band and the even write band immediately. Checking minimal write removes the full write wash and shows the bar. The pair persists across reload.

### Table cache

The age line matches the side chip ([07-tables.md](07-tables.md)). Before any list has been fetched, including while the window is still in Query mode, the line is **not cached** and the title is **No table list cached.**

**Clear table cache** clears the cache, refetches the list, sets the note **Table cache cleared.**, and the age to just now. The tooltip after a fetch is **Fetched the table list.**

### Keyboard

| Action | Keys |
| --- | --- |
| **Run selection** | **Ctrl + Enter** and **Ctrl + \\** (⌘ on macOS) |
| **Stop query** | **Esc** |

The labels are visible text, not only tooltips.

## Theme

The theme button toggles light and dark. `aria-pressed` follows the current theme. The label is **Switch to light mode** when the theme is dark, and **Switch to dark mode** when it is light. The icon is the moon in light mode and the sun in dark mode. `body` background is the paper token. The choice persists and survives reload. The header bar stays dark in both themes.
