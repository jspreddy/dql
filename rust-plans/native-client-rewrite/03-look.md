# Look

The page is the picture. Stock toolkit themes (Catppuccin, Dracula, a default light theme) are not this product. The header, key washes, chips, and write bands are extra tokens beyond a six-color palette.

Type: UI is Segoe UI, system-ui, sans-serif, 13px, line-height 1.35. Code is ui-monospace, SFMono-Regular, Menlo, Consolas. The page editor is 12.5px with line-height 1.4. The iced window uses 13px mono and an 18px line. Match the page: 12.5px mono in the editor, 13px sans everywhere else, unless a control below names a size.

Icons are the same strokes as the page, drawn in the widget’s own paint. They are listed in [12-toolkit.md](12-toolkit.md) because a separate geometry pass left them stuck on screen.

## Light and dark

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| bar | `#1b2838` | `#0c1117` | Header. Dark in both themes. |
| header fg | `#e8eef5` | `#e8eef5` | Wordmark |
| header text | `#d5dde6` | `#c9d4e0` | Header controls |
| header line | `#45566b` | `#3a4d63` | Header hairlines |
| teal | `#0f766e` | `#0f766e` | Selected mode, open-file mark, links |
| teal ink | `#0f766e` | `#5eead4` | Kickers, help keywords |
| text | `#1c2430` | `#e7eef6` | Body |
| muted | `#667484` | `#8b9eb3` | Secondary labels, icons |
| side | `#f3f5f7` | `#121922` | Side column |
| paper | `#ffffff` | `#17202b` | Editor, cards |
| canvas | `#f7f9fb` | `#0e141b` | Results ground |
| line | `#e4e7eb` | `#2a3848` | Hairlines |
| line strong | `#e1e6eb` | `#334556` | Stronger rules |
| blue | `#2563eb` | `#60a5fa` | Run frame, primary |
| play | `#2563eb` | `#2563eb` | Gutter play circle. Hover `#1d4ed8` / `#3b82f6`. |
| danger | `#b91c1c` | `#fca5a5` | Errors, Run file, destructive menu items |
| ok | `#166534` | `#86efac` | Success |
| ok bg | `#f0fdf4` | `#142a1e` | Success chip |
| amber | `#92400e` | `#fde68a` | Warnings |
| amber bg | `#fffbeb` | `#3a2e14` | Warning chip |
| status | `#f1f5f9` | `#141c26` | Footer |
| gutter | `#fafbfc` | `#141c26` | Line-number column |
| chip | `#e8eef5` | `#2a3a4e` | Count pills |
| selected | `#d9e6f6` | `#1c3d5c` | Selected tree or table row |
| stripe | page `--stripe` | page `--stripe` | Even result rows |
| hover teal | page `--hover-teal` | page `--hover-teal` | Row hover |

Selection in the editor is `rgba(37, 99, 235, 0.28)` light and `rgba(96, 165, 250, 0.34)` dark. The run frame fill is `rgba(16, 42, 96, 0.05)` light and `rgba(96, 165, 250, 0.14)` dark, with a 1px `--blue` border and a 5px gap outside the text.

Modal scrim is `rgba(15, 23, 42, 0.38)`. Dropdowns (search, settings, environment, menus) use a 10px radius and a soft shadow (`0 12px 32px` on the settings card). Full-window sheets have square corners and fill the window. Small buttons use a 4px radius. Pills (search, endpoint, count) use a full pill radius.

## Syntax

Keywords are bold. The page maps tokens through `dql-tokens.js` onto `--tok-*`. The iced window simplified that set. The rewrite matches the page tokens.

| Token | Light hex | Weight |
| --- | --- | --- |
| keyword | `#0f766e` | bold |
| type | `#1d4ed8` | regular |
| function | `#6d28d9` | regular |
| string | `#9f1239` | regular |
| number | `#b45309` | regular |
| comment | `#6b7280` | regular |
| deleted / write keyword | `#dc2626` | bold |

A write statement is one whose action word is `INSERT`, `UPDATE`, `DELETE`, `DROP`, `CREATE`, `ALTER`, or `LOAD`. The first keyword of that statement is the write color and bold. Other keywords on the line stay the keyword color and bold. Strings, comments, and identifiers stay regular weight. Comments are not italic.

The iced highlighter used `#b91c1c` for the write word and `#b45309` for strings. Those are close and wrong. Use the page tokens above.

## Write bands

Read statements alternate a plain line and an alt wash (`#f4f7f8` light, `#1e2936` dark) when even/odd highlight is on. Even/odd off paints every read line plain.

Write statements use a full-line wash: `#fdecec` / `#3a1d24`, and the even write `#f3c4c4` / `#5a2832`. Minimal write highlight replaces the wash with a 3px gutter bar: `#fb7185`, and the even bar `#be123c` light / `#fda4af` dark.

## Column washes

Key columns are tinted, not labeled with the words HASH and RANGE. The mark is an icon.

| Role | Mark | Head wash |
| --- | --- | --- |
| Table hash | hash grid | `--col-table-head` |
| Table range | range arrows | range wash |
| Index hash | hash grid | index wash |
| Index range | range arrows | index wash |
| Both (table and index) | both marks | `--col-both` |

Shared-key tooltips, from the page tests: **Table hash key**, **Index range key**, **Index hash key**. Borders: hash `#6fbf96`, range `#7aa6e0`.

## Icons

Drawn at the size of the control, recolored to the current ink (muted `#667484` on light cards, teal for the row eye, white on the play circle, danger on Run file).

| Mark | Where |
| --- | --- |
| Settings (three lines, three knobs) | Header settings |
| Moon | Theme, when the theme is light (click switches to dark) |
| Sun | Theme, when the theme is dark |
| Refresh | Tables side head |
| Fast-forward (two triangles) | Run file, and Run file / Run folder in the menu |
| Play (one triangle) | Header Run, and the gutter circle |
| Table grid | Schema button on a result card and on the table page |
| Eye | Row peek, 14px, teal |
| Hash | Key chips and column heads |
| Range | Key chips and column heads |
| Info | Result structure sheet only |
| Close | Sheets and dialogs |
| Trash | Clear results |

The page also uses a magnifying glass on intelligent-match notes and a clock on describe cache hints. Include those where those notes appear.

Tree disclosure is the text marks `▸` and `▾`, not an SVG. Run status in the gutter is `✓` and `×`, and a spinner while that statement runs. Lint gutter marks are `!` and `●`.

## Controls

| Control | Size |
| --- | --- |
| Header | 36px |
| Search field | 24px |
| Theme and settings | 26×22 |
| Mode, Run, Stop, side actions | 22px tall |
| Tree row | 22px |
| Gutter play | 22×22 circle |
| Clear table cache | 26px |
| Results head | at least 36px |
| Result count chip | 22px pill |
| Row peek column | 28px, sticky to the left edge |

Button labels sit on the vertical center of the control. The iced window had labels sitting high inside the button; that is not the look. Result count pills and timing pills use the same alignment.

Run file is a danger-colored outline button with a paper fill, sitting in the editor head to the right of the file name, vertically aligned with the name within 8px. Its icon is the fast-forward mark.

## Sheets and cards

Structure and row sheets fill the window. The card has no radius. Inner blocks (query, JSON, the column grid) use an 8px radius. Result tables use an 8px outer radius. The patch dialog and any small modal use a 14px radius. Those two are not full-window.

Section labels in settings and on sheets are 11px, semibold, uppercase, with slight tracking (the schema kicker uses `0.08em`). The schema kicker is teal ink. The sheet title is 16px bold. JSON and the query block are 12.5px mono.
