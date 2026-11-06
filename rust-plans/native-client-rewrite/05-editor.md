# Editor

The editor is a monospace buffer with a gutter, a statement frame, syntax color, write bands, lint marks, and completion. Padding is about one line above the first line and about five lines below the last (ratios 0.85–1.2 and 4.7–5.3 against the line height).

The gutter shows line numbers and, per statement, a running spinner, `✓`, or `×`. Lint uses `●` or `!` in the gutter. Numbers scroll with the text. A gutter that stays still while the buffer scrolls is not this editor.

## What Run uses

`runTarget(text, head, anchor)` in `dql-tokens.js`:

1. If the selection is non-empty after trim, the run is that selection, expanded to the full lines it touches.
2. Otherwise the cursor is tested against the query bands. A cursor on the band, or at the end of a band that ends at EOF, runs that band.
3. Otherwise there is nothing to run. The results meta reads **Nothing to run**. The iced status line also uses **No statement at the cursor.** Prefer the page string in the results meta.

The frame paints the run range: a blue outline and the run-fill wash, one frame only. The fill is inset so a range that ends at the start of the next line does not paint that next line. The selection layer sits above the band wash, which sits above the run fill.

## Bands

`queryBands` scans the buffer, skipping `--` comments and quoted strings (backslash escapes). The action word is the first word, or the first word after `EXPLAIN` or `ANALYZE`. A band ends at `;`.

`expandQueryBands` then grows the band:

- Upward over comment-only lines that touch the statement and are not already claimed.
- Downward over comment-only lines, then over the blank lines that follow, until the next claimed line or EOF.
- A blank line belongs to the statement above. It is the boundary before the next statement. The next statement’s frame overlaps that blank by only a couple of pixels, not a full line.

Worked example from `queries/read.dql`: a header comment `-- header stays with the select`, the `SELECT` through `WHERE id = 'a'`, then `-- next`, then `SCAN`. A cursor in `WHERE id = 'a'` frames the header comment, the SELECT line, and the WHERE line (more than 80% of each line). `-- next` overlaps by 2–12px. The SCAN line overlaps by less than 2px. A drag from SELECT through WHERE frames the same span. A drag that is only part of a line still frames the full lines, and the native selection highlight stays visible inside the frame.

A selection that contains non-whitespace wins over the band. The play button and Run execute the selection, not the surrounding statement.

Quoted semicolons and comment semicolons do not end a statement. A cursor at EOF can run a trailing statement that has no final semicolon only when the band scan includes it; statement spans for Run file still split on `;`.

## Run file versus the frame

Run file uses `statementSpans`: split on `;` outside strings and comments, drop a span whose lines are all comments or blank, trim leading space and trailing space. A file run executes every span. The gutter play button and the header **Run** execute `runTarget` only.

## Syntax and bands

Color follows [03-look.md](03-look.md). Keywords are bold, including `SELECT`, `FROM`, `WHERE`, and the write words. `CREATE`, `INSERT`, `UPDATE`, `DELETE`, `DROP`, `ALTER`, and `LOAD` as the action word are the write red and bold.

Highlight settings change the washes, not the token colors. The matrix is the one `bandAppearance` implements: minimal write replaces the write wash with the gutter bar; even/odd off removes the alt wash and the even write wash. Both toggles persist and survive reload.

## Lint

Lint runs on a short debounce (the page uses 400ms) over the buffer. It does not wait for save.

| State | Chip | Gutter | Color |
| --- | --- | --- | --- |
| `SELEKT * FROM shelf;` | **1 error**, tooltip matches the message (expected a DQL statement) | error mark | `--danger` wash and a squiggle |
| Unknown table | **1 warning**, class warning, “no cached definition” | warning mark | amber |
| Clean | chip hidden | no marks | |

The squiggle is on the offending span in the buffer. A list under the editor is not a substitute. Hovering the mark shows the message. Clicking the chip opens the Lint tab (see [06-run-and-results.md](06-run-and-results.md)) and focuses the line.

The iced highlighter cannot paint a ranged squiggle, so the iced window lists the diagnostic under the editor and puts a dot in the gutter. The rewrite paints the squiggle.

## Completion

Completion looks at the statement since the previous `;`. After `FROM` or `INTO` it offers table names. After `WHERE`, `AND`, or `OR` it offers key names. The list shows up to 50 rows. Arrow keys move, Tab and Enter insert the active row, Escape dismisses. The list is anchored at the caret, above later clicks, and it does not push the buffer down.

## Keys while the editor is focused

These are editor bindings, not a window-level shortcut that only works when the editor happens to see the event:

| Chord | Action |
| --- | --- |
| Ctrl+Enter or ⌘+Enter | Run selection |
| Ctrl+\ or ⌘+\ | Run selection |
| Ctrl+S or ⌘+S | Save now |
| Escape | Stop, after the overlay rules in [10-interaction.md](10-interaction.md) |

On macOS the visible label uses `⌘`. The Run button’s shortcut hint reads **Ctrl + Enter** or **⌘ + Enter**, and `aria-keyshortcuts` lists both Enter and backslash.
