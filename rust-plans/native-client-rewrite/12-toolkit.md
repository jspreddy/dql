# Toolkit constraints

These are requirements on the Rust GUI, learned by building the iced window. They are not a choice of crate. egui, slint, gpui, iced, or something else can host the window if it can do the following. A webview around the current page cannot: it keeps JavaScript and a bridge.

## Paint and damage

Icons are part of the widget that owns them. On the tiny-skia backend, a stroked canvas path reported bounds with no stroke width. Lines have an empty box. The software presenter clears only that box, so the ink stays where it was first drawn. Further interaction paints new copies and leaves the old ones. A backend that clears the full frame hides the bug. Windows often falls back to the software presenter, so the bug is the common case.

The rewrite draws icons as images, or as filled geometry whose damage rectangle is the widget’s rectangle, including the stroke. Moving, scrolling, theming, or closing a sheet clears the previous pixels. A still frame after the sheet closes shows no extra dark pixels where the close icon, the info icon, or the schema icon were.

The same rule applies to the gutter play button, the hash and range marks, the eye, and the header icons.

## Overlays

Floating widgets paint above ordinary later siblings. A full-window sheet that is only “the next child” ends up under the play button. The sheet is an overlay with a higher order than every float, and it captures pointer events so a press cannot land on a control underneath.

Hiding the play button while a sheet, the settings card, or the search list is open is still required. Hit testing can visit lower floats even when they paint underneath.

Menus and the search list float. Inserting them into the column pushes the editor and resets scroll. The menu anchor is the pointer position at the right-click and does not follow later moves.

## Editor

The toolkit needs a text buffer with:

- A caret, a selection, and per-span color, including bold and a second color on the same line.
- A squiggle on an arbitrary byte range for lint. A highlighter that can only color a whole line is not enough. The iced `text_editor` highlighter is that case.
- Line numbers and status marks that scroll with the buffer. A sibling column that does not share the editor’s scroll is not enough.
- Command bindings that fire while the editor is focused and do not insert the chord as text.
- Enough bottom padding to show the last lines above the results split.

## Scrolling grids

Result grids and the table page scroll on both axes. The header sticks vertically. The peek column sticks horizontally. The header’s horizontal offset tracks the body. Nested scrollables that lay out at zero height are a known failure; one scroller for the body, and a header synced to it, is the arrangement that worked.

Buttons inside a scroller, including the eye, receive the press. The scrollbar’s hit area does not cover the eye.

## Splits

Two splits, drag and keyboard, persisted as pixels, double-click to reset, minimum sizes in [02-shell.md](02-shell.md). No close, maximize, or arbitrary nest.

## Lists

Rows are keyed by identity. Rebuilding the list does not reuse one row’s focus or scroll state for another. Fifty table rows and the result history stay keyed. Virtualization is optional at that size.

## Theme

The toolkit’s stock palette is not the product palette. The window supplies paper, text, teal, ok, amber, and danger, and it paints the header, chips, key washes, and write bands from [03-look.md](03-look.md). Light and dark switch without restarting. The header stays dark either way.

## Input

Mouse, keyboard, drag and drop of files onto the tree, and a directory watch on a background task. Run and table refresh are cancellable tasks. Short work (toggle, menu, theme) happens on the UI thread.

## Tests without a display

The toolkit needs a headless harness: build the view, click, type, and find a widget by text or id, with no window server. That harness replaces Playwright for the window. A software renderer can write a PNG for the few layout shots. A missing baseline fails the test. Those shots do not replace the behavioral assertions.

## What not to take from iced

| Iced limit | Requirement instead |
| --- | --- |
| Canvas icons ghost on tiny-skia | Image or filled geometry with honest damage |
| Floats paint over later siblings | Sheets are a higher overlay and eat clicks; hide the play button while they are open |
| Highlighter cannot squiggle | Lint underline on a byte range |
| Gutter cannot follow the editor scroll | Marks scroll with the text |
| No file drag and drop | Drag onto a folder and onto the tree root |
| Tooltip overlay can swallow a click | The control under the tip still activates |
| `opaque` captures presses but is not an overlay | A scrim that only captures presses still paints under floats |
