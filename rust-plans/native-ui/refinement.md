# Plan: native UI refinement

**Status:** in the window. `dqlrs --ui` on iced 0.14.0 (`advanced`, `tokio`) now drags its splits, floats menus and search, scrolls wide grids on both axes, and toasts save and run failure. Behavior stays the list in [`prd.md`](prd.md). `dqlrs-web` stays.

The source is the [iced 0.14.0 tree](https://github.com/iced-rs/iced/tree/0.14.0), not `master`. `examples/README.md` on that tag still describes tour, todos, and styling, and it omits later examples that are in the tree (`modal`, `toast`, `table`, `lazy`, `editor`, `scrollable`). The widget list is [`widget/src/lib.rs`](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/lib.rs). The book page cited by [`testing.md`](testing.md) is the architecture (state, message, `update`, `view`). It does not add widgets.

## Already in the window

`button`, `column`, `row`, `container`, `text`, `text_input`, `text_editor` with `DqlHighlighter`, `scrollable` (vertical only), `mouse_area`, `stack`, `Space`, and `Shadow`. Keyboard input is a `Subscription` on key presses. Theme is `Theme::Light` or `Theme::Dark`, and the chrome is a separate palette painted in `style.rs`.

That set covers the shell. The gaps below are places where an iced 0.14 example already shows a better interaction than the one we built by hand.

## 1. Drag the splits

[`examples/pane_grid`](https://github.com/iced-rs/iced/blob/0.14.0/examples/pane_grid/src/main.rs) builds a `pane_grid::State`, draws `PaneGrid`, and handles `ResizeEvent { split, ratio }` from `on_resize`. The sidebar width and the results height are already `prefs.nav` and `prefs.results`, changed by the `‹` `›` and `–` `+` buttons.

Build one vertical split (tree or table list, then the main column) and one horizontal split inside the main column (editor, then results). Handle `Resized` and write the ratio back into those prefs. Keep the panes fixed: no split, drop, maximize, or close. The example's `responsive` closure is only useful later, if a narrow pane should drop button labels.

## 2. Scroll a wide grid on both axes

[`examples/scrollable`](https://github.com/iced-rs/iced/blob/0.14.0/examples/scrollable/src/main.rs) sets `scrollable::Direction::Both { vertical, horizontal }` on one `scrollable`. A nested horizontal `scrollable` inside the results scroller laid out at 0 height, so wide result grids clip.

Put the result grid and the table-data grid in one `scrollable` with `Direction::Both`. Give each scroller a `widget::Id`. `on_scroll` reports a `Viewport`. `operation::snap_to(id, offset)` moves a scroller, which is how the example's buttons jump. Use that pair to keep a header row and the body on the same x offset: the header is its own horizontal scroller, the body scrolls both ways, and `on_scroll` copies `absolute_offset().x`.

[`widget::table`](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/table.rs) (`examples/table`) is a cell grid with padding, alignment, and separators. It lays out every cell. It has no row virtualization, and its header is the first row, so it scrolls away. Keep our grid (key washes, peek cell, bottom rule). Take `Direction::Both` and the synced header. Revisit `table` only if a spike can paint those three things with `table::column`.

## 3. Overlays that eat clicks and stay on screen

[`examples/modal`](https://github.com/iced-rs/iced/blob/0.14.0/examples/modal/src/main.rs) is a `stack` of the window and an `opaque` layer. `opaque` ([`helpers.rs`](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/helpers.rs)) captures mouse presses so they do not fall through to the window. The scrim is a `mouse_area` whose `on_press` closes the dialog. Opening the dialog returns `operation::focus_next()`. Tab and Shift+Tab call `operation::focus_next` and `operation::focus_previous`.

Our row and structure dialogs are a `stack` without `opaque`, and a click on the card background can hit the scrim. Wrap the card and the scrim with `opaque`, and focus the close control when the dialog opens.

The file menu is inserted under the tree row, and the search list is inserted under the header, so both push the layout. [`widget::float`](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/float.rs) takes a `translate` closure of content bounds and viewport bounds and returns a `Vector`, which is how a menu stays inside the window. `mouse_area::on_right_press` carries no point. `on_move` does, and so does `Event::Mouse` from [`examples/events`](https://github.com/iced-rs/iced/blob/0.14.0/examples/events/src/main.rs). Store the last cursor point, and on right-click place the menu with `float`. Use the same overlay for the search groups, anchored under the search field.

[`widget::rule`](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/rule.rs) is the 1px separator the menu and the dialogs currently fake with a 1px `container`. The styling example uses it between sections.

## 4. Editor keys and the real cursor

[`examples/editor`](https://github.com/iced-rs/iced/blob/0.14.0/examples/editor/src/main.rs) reads `content.cursor().position` (line and column), `content.text()`, and uses `key_binding` so a command chord returns `text_editor::Binding::Custom(Message)` instead of a text edit. Our cursor is parsed back out of `Debug`. `editor_text` joins `content.lines()` by hand. `Content::text()` and `Content::selection()` are on the widget.

Move Ctrl+Enter, Ctrl+\, and Ctrl+S into `key_binding` on the open editor so they apply while the editor is focused. Keep the window subscription for Escape when a dialog or menu is open.

`text_editor` scrolls internally (`partial_scroll` is private). A sibling line-number column cannot follow that scroll. Leave the gutter clipped. Drawing numbers inside the editor's scroll is a custom widget ([`examples/custom_widget`](https://github.com/iced-rs/iced/blob/0.14.0/examples/custom_widget/src/main.rs)), and the line highlighter still cannot paint a lint squiggle on a byte range. The gutter dot stays.

## 5. Run feedback

[`examples/progress_bar`](https://github.com/iced-rs/iced/blob/0.14.0/examples/progress_bar/src/main.rs) is `progress_bar(range, value)`. The run line prints a text bar. Replace that string with `progress_bar` when `live.total` is known. Style it with the teal from the palette.

[`examples/tooltip`](https://github.com/iced-rs/iced/blob/0.14.0/examples/tooltip/src/main.rs) wraps a widget with `tooltip(target, tip, Position)` and an optional delay. Put tips on the icon buttons (pane steps until step 1 lands, schema, the row peek, theme, settings). `Position::FollowCursor` is the variant for a tip that tracks the pointer.

[`examples/toast`](https://github.com/iced-rs/iced/tree/0.14.0/examples/toast) is not a built-in widget. It is a `Vec` of cards drawn in a `stack` above the window, each with a title, a body, and a status color, removed on a timer. Use that for save, conflict, and run failure. The status bar stays for the idle line.

## 6. One palette for the stock widgets

[`examples/styling`](https://github.com/iced-rs/iced/blob/0.14.0/examples/styling/src/main.rs) switches `Theme` and uses `button::primary`, `button::danger`, `button::success`. [`Theme::custom`](https://github.com/iced-rs/iced/blob/0.14.0/core/src/theme.rs) takes a name and a [`Palette`](https://github.com/iced-rs/iced/blob/0.14.0/core/src/theme/palette.rs) of six colors: background, text, primary, success, warning, danger.

Point those at paper, text, teal, ok, amber, and danger, and return that `Theme` from `App::theme` for light and dark. The editor background, selection, and scrollbars then follow the same colors as the chrome. `Palette` has no field for the header bar, key washes, or chips. Those stay in `style.rs`. Built-in themes (`Catppuccin`, `Dracula`, the rest of `Theme::ALL`) do not match the web tokens.

## 7. Lists that keep their place

[`widget::keyed`](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/keyed.rs) asks for a stable hint per child so focus and scroll survive a rebuild. Result history and the table list are rebuilt from scratch on every `view`. Key the result cards by statement index and the table rows by name.

The [`lazy` example](https://github.com/iced-rs/iced/blob/0.14.0/examples/lazy/src/main.rs) needs the `lazy` feature. `lazy` skips rebuilding a child when its dependency is unchanged. It does not virtualize. Pages stay at 50 rows, and the result list stays capped. Turn `lazy` on only if a keyed list still rebuilds too much of a 50-row page.

## Examples that do not change this window

| Example or widget | What it is |
| --- | --- |
| `canvas`, `svg`, `shader`, `qr_code`, `image`, `geometry`, `clock`, `solar_system`, `game_of_life`, `bezier_tool` | Drawing. Text marks stay. `svg` is the later fix if a glyph is missing from the font. |
| `markdown` | A viewer that needs the `markdown` and `image` features. HELP is already plain text. |
| `combo_box`, `pick_list`, `slider`, `checkbox`, `toggler`, `radio`, `todos` | Controls we do not have on the page. Dialogs stay text inputs. |
| `multi_window` | One window. |
| `highlighter` feature | Syntect for other languages. `DqlHighlighter` stays. |
| `screenshot`, `integration`, `websocket`, `download_progress`, `system_information`, `tour` | Harness, embedding, and demos. Screen tests stay in [`testing.md`](testing.md). |
| `grid` | A wrap of children into columns. It is not a data table. |
| `pin` | Places one widget at a fixed point. It does not stick a header to a scroller. |
| `sensor` | Fires when a widget enters the viewport. Paging stays on Previous and Next. |

## Order

1. `opaque` dialogs, `float` menus and search, `rule` separators.
2. `Direction::Both` on the result grid and the table grid, with a header that shares x.
3. `pane_grid` resize written back to `prefs.nav` and `prefs.results`.
4. `Content::text`, `cursor().position`, and `key_binding`. Then `Theme::custom` for the six stock colors.
5. `progress_bar`, `tooltip`, and a toast stack for save and run failure.
6. `keyed` result cards and table rows.

Each step keeps the headless clicks on `Files`, `New file`, `id`, and `a`.

## Out of this plan

- Features the web page does not have.
- Removing `dqlrs-web`.
- Tracking iced `master`.
- A virtualized table. iced 0.14 `table` and `lazy` both build the cells they are given.
