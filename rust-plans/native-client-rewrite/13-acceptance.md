# Acceptance

A behavior is required when the page implements it and a Playwright spec locks it, or when this folder specifies it because the iced window had to match the page and the spec does not spell the pixel rule out. The iced tests lock the subset the iced window actually built. They are not the ceiling.

Headless tests drive the studio and the widget tree. They find controls by their visible text: **Files**, **New file**, **Query**, **Tables**, **No matches**, **RESULT STRUCTURE**, **ROW**, **Table cache cleared.** Screenshot tests are extra. They do not replace these assertions.

## Playwright map

| Spec | Required behavior |
| --- | --- |
| `files.spec.js` | Side resize, persist, arrow nudge, double-click reset to about 220px. Tree contents, collapse, selection, open-file border and underline. One running dot, then none. File menu items and the root menu’s limits. New file, new folder, duplicate, rename with Escape, move by prompt and by drag, delete, refuse a folder that contains `.gitkeep`. Autosave. Run file and run empty folder (**Nothing to run**). |
| `editor.spec.js` | Theme persist, paper background, results resize and the 46% reset, editor padding. Endpoint chip. Run label, shortcut, tooltip. Query frame including comments and the following blank line. Selection wins. Highlight toggles. Ctrl+Enter and Ctrl+\\. Settings shortcut list. Run file versus History. Run folder isolates per-file results. Clear results for a file and a folder. Result, History, and Lint tabs. Row sheet title and chips. Card meta, capacity, time. ANALYZE capacity. Zero capacity omits the chip. History pin. Query chevron. EXPLAIN fields and the raw toggle. Stop and **Query stopped**. Progress to completion. Gutter play. `SHOW TABLES` throughput. Intelligent match copy. Describe card. Completion, Escape, Tab. Scroll to the failing statement. Sealed query-result cache. |
| `tables.spec.js` | Tables mode chrome. Refresh versus filter. Cache age. Clear cache **Table cache cleared.** Settings left of theme. **not cached** before a fetch. Pager **Showing 1–50**. Key icons. Sticky header and sticky peek. Column order and role tooltips. Similar names and related keys. Both structure sheets, Esc, no extra query. Wide horizontal scroll. |
| `search.spec.js` | Field placement. Query hit opens the statement. Column, index, and table hits open Tables mode. Listed-only table has no column hits. **No matches**. |
| `save-load.spec.js` | SAVE and LOAD from the process cwd, JSON and CSV, absolute paths, `CREATE PARENTS`, the missing-parent OS error. |
| `lint.spec.js` | Error and warning chips, gutter, squiggle colors, hover tip, Lint tab, click focuses the line. |
| `help.spec.js` | **HELP select**, **Available commands**, keyword color, example text. |
| `functions.spec.js` | The functions script runs clean and the listed functions return the expected cells. |
| `nested.spec.js` | Help examples for nested paths run and return one row. |
| `file-watch.spec.js` | Clean external edit, patch dialog **File changed on disk**, recall does not write the disk, two windows. |

Presence and **The server went away** are in `files.spec.js` and `file-watch.spec.js`. They are web-only. See [01-objective.md](01-objective.md).

## Iced tests that stay green in spirit

These live in `rust-impl/crates/dql-cli/src/dqlrs_ui/`. A new toolkit rewrites the harness. The assertions stay.

| Test | Assertion |
| --- | --- |
| `comments_and_blank_lines_follow_the_web_bands` | Band edges for comments, blanks, writes, quotes, EOF. |
| `a_non_whitespace_selection_wins_over_the_band` | Selection versus the band. |
| `the_play_button_sits_left_of_the_frame_and_hides_when_scrolled_away` | Circle just left of the frame; gone when the frame is off screen. |
| `highlight_options_paint_like_the_web_menu` | The four combinations of minimal write and even/odd. |
| `colors_keywords_strings_and_write_statements` | Keyword, write word, string, comment, bold on keyword and write. |
| `run_cursor_includes_attributed_comments_and_keeps_a_selection` | The cursor run includes the leading comment. A selection runs the selection. |
| `window_covers_file_menu_run_conflict_and_theme` | New file, lint, run, conflict, foreign-folder delete, theme persist, clear cache, Tables mode. |
| `title_search_opens_the_statement_and_keeps_the_query` | 150ms debounce, open the statement, Escape keeps the text, whitespace clears it. |
| `the_run_button_stays_under_settings_and_search` | The play button is suppressed while settings or search is open. |
| `structure_and_row_popups_follow_the_web` | Both kickers, the footers, the row title, sheets cover the editor, Escape closes the row. |
| `cache_age_matches_the_web_settings_label` | **not cached** through **2 days ago**. |
| `table_name_skips_a_leading_comment` | A leading `--` line still yields the table. |
| `row_json_follows_column_order` | `id`, then `sku`, then `name`. |
| `growing_table_list_keeps_each_row_state` | A longer catalog does not corrupt row state. |
| `nav_click_keeps_menu_anchor_and_widget_state` | The menu anchor stays at the right-click. |

## Where the iced window is behind the page

The rewrite does not treat these as done.

| Gap | Page behavior |
| --- | --- |
| Result, History, Lint tabs | [06-run-and-results.md](06-run-and-results.md) |
| Lint squiggle and click-to-line | [05-editor.md](05-editor.md) |
| Completion list | [05-editor.md](05-editor.md) |
| Drag and drop in the tree | [04-files.md](04-files.md) |
| Directory collapse | [04-files.md](04-files.md) |
| File menu **New folder**, **Clear results**, label **Move to…** | [04-files.md](04-files.md) |
| Patch dialog with a diff | [04-files.md](04-files.md) |
| Scroll the failing statement into view | [06-run-and-results.md](06-run-and-results.md) |
| Per-statement gutter `✓` / `×` | [06-run-and-results.md](06-run-and-results.md) |
| Help card and EXPLAIN layout | [06-run-and-results.md](06-run-and-results.md) |
| Describe card and intelligent-match copy | [06-run-and-results.md](06-run-and-results.md) |
| Page syntax tokens (`#9f1239` strings, `#dc2626` write) | [03-look.md](03-look.md) |
| Icon damage on a software presenter | [12-toolkit.md](12-toolkit.md) |

## Visual checks

On a software presenter (`ICED_BACKEND=tiny-skia` was the iced case; the next toolkit names its own):

- Open the result structure sheet, close it, and compare the frame. The close glyph’s pixels are gone. No new icon-colored pixels sit on the editor.
- Scroll a result grid. Hash, range, and eye icons move with their rows. The old positions show the row wash, not a second icon.
- Open settings and the search list. The gutter play button is not on top of either.
- Light and dark: header stays `#1b2838` or `#0c1117`, paper and text follow the table in [03-look.md](03-look.md).

## Command

`just verify` grows a native-client suite that runs without Playwright and without a display. Playwright stays until that suite covers the map above. The suite does not keep a browser in the loop after the cutover.
