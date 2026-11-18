# DQLRS Web design guidelines

How the web app in `static/` should look and behave, and how to add to it. Read this before you add a screen, a control, or a color.

## Principles

1. **Dense, not cramped.** This is a tool people keep open next to a terminal. Rows are 22px, the base text is 13px, and the chrome stays out of the way of queries and data.
2. **Data first.** The editor and the result table get the room. Decoration (borders, fills, shadows) only separates regions or states a meaning.
3. **Color means something.** Teal is the app and the primary action. Red is a write or a destructive action. Blue is selection and "this is running". Green is success. Amber is a warning. Don't use any of them for decoration.
4. **Light and dark are equal.** Every screen ships in both. A change that isn't checked in both isn't done.
5. **Keyboard first.** Everything reachable by mouse is reachable by keyboard, and focus is always visible.

## Files

| File | Holds |
| --- | --- |
| `static/theme.css` | Color tokens, light and dark. The only place a color value is written. |
| `static/components.css` | Shared components: button, icon button, dialog. |
| `static/app.css` | Layout and screen-specific styles. |
| `static/*.js` (`dialogs.js`, `context-menu.js`, `run-tree.js`, …) | Script components, one module each. |
| `playwright/tests/components/` | One spec per component, run in the bare harness. |
| `playwright/tests/design-review.spec.js` | One screenshot of every main screen, light and dark. |

## Color

- Use a token from `theme.css`: `color: var(--muted)`. Never write a hex or `rgb()` value in `app.css`, `components.css`, or script.
- A new color is a new token with a light value and a dark value. Name it for its role (`--danger-wash`), not its hue.
- Check the dark value against its own background; don't invert the light one.

| Role | Tokens |
| --- | --- |
| Surfaces | `--paper` (content), `--side` (navigation), `--canvas`, `--bar` (top bar) |
| Text | `--text`, `--text-strong`, `--label` (headings of small groups), `--muted` (secondary) |
| Lines | `--line` (between regions), `--line-control` (around controls) |
| Primary action, brand | `--teal`, `--on-fill` for text on it |
| Danger, writes | `--danger-fill`, `--danger`, `--write` |
| Selection, running | `--blue`, `--selected`, `--selection` |
| Success | `--ok`, `--ok-bg` |
| Warning | `--amber`, `--amber-bg` |
| Hover | `--hover`, `--hover-quiet`, `--chip-hover` |
| Shadows | `--shadow` (raised), `--shadow-pop` (popover), `--shadow-lg` (dialog), `--shadow-modal` (backdrop) |

## Type

- Noto Sans is bundled in `static/vendor/fonts/` (Latin and Latin Extended, variable weight). The app loads nothing from outside, so it works offline. Don't link a font or any other asset from a CDN.
- Two families: `var(--font)` (Noto Sans) for the interface, `var(--mono)` for queries, keys, values, and file paths.
- Use this scale. Half sizes (`12.5px`, `13.5px`) exist in older rules; don't add more.

| Size | Use |
| --- | --- |
| 10px to 11px | Uppercase group labels, chips, table type tags |
| 12px | Secondary text, metadata, result cells |
| 13px | Base: tree, toolbar, body text |
| 16px | Dialog titles |

- Weights: 400 for text, 650 for controls and headings, 700 for dialog titles.
- Uppercase is only for small group labels (`TABLE CACHE`), with letter spacing.

## Space and shape

- Spacing steps: 4, 8, 12, 16, 22px. Controls in a row sit 8px apart.
- Heights: 22px for a row or a toolbar control, 28px for an icon button, 32px for a dialog button or input.
- Radius: 4px for toolbar controls and cells, 8px for buttons and inputs in dialogs, 14px for a dialog card, 999px for chips and pills.
- A border separates. A shadow lifts something above the page (menus, popovers, dialogs). Don't use both to say the same thing.

## Components

Compose the shared classes. Don't style a button or a dialog by its id.

### Button (`.btn`)

The action in a dialog or a form. 32px tall.

| Class | Use |
| --- | --- |
| `.btn` | Quiet (secondary): Cancel, Close, Previous, Next |
| `.btn.btn-primary` | The one action the dialog exists for: Create, Run, Retry. At most one per dialog. |
| `.btn.btn-danger` | Deletes, overwrites, or stops work: Delete, Stop |

- Label with a verb that says what happens: "Delete", "Run 1 file". Not "OK" or "Yes".
- Order: quiet actions first, the primary action last (bottom right). An action bar at the top of a sheet (the run dialog) keeps that order but aligns left.
- A button with a keyboard shortcut shows it after the label: `<span class="btn-label">Run</span><kbd class="btn-kbd">Esc</kbd>`, plus `aria-keyshortcuts`. Use a `.key-mod` span for Ctrl, so a Mac shows ⌘.
- A destructive confirm focuses Cancel, not the destructive button.

### Input group (`.input-group`)

A leading label and one control in a single 24px bordered box, read as one phrase: `Index [ -- ▾ ]` in the table browser. The label (`.input-group-text`) sits on the chip fill; the control has no border of its own. Focus rings the whole group.

```html
<label class="input-group"><span class="input-group-text">Index</span><select aria-label="Table or index">…</select></label>
```

Use it instead of a separate uppercase field label when the label is a word that leads into the value.

### Icon button (`.icon-btn`)

A 28px square with one 12 to 14px icon, such as a dialog's close. It needs an `aria-label`. Add `data-tip` when the icon alone doesn't say what it does.

### Dialog (`.dialog`)

A native `<dialog>` opened with `showModal()`. Escape and a click on the backdrop cancel.

| Class | Use |
| --- | --- |
| `.dialog` + `.dialog-card` | A centered card for a question or a short task. 420px wide; set `--dialog-width` for more. |
| `.dialog.dialog-sheet` | Fills the window, for content that needs the room: a table's structure, a row, a run. |
| `.dialog-title` | The `<h2>` at the top. Reference it with `aria-labelledby`. |
| `.dialog-actions` | The button row at the bottom right. |

Ask with `createConfirmDialog` and `createPromptDialog` from `dialogs.js`. Never use `window.confirm` or `window.prompt`.

### Toolbar controls

The 22px controls in the top bar, the side header, the pane headers, and the pager are styled in `app.css` (`.side-head button`, `.pager button`, `.primary`, `.file-filter`). They are outlined, 4px radius, and regular weight, so they read as chrome, not as dialog actions.

### Tooltips

One tooltip (`#lint-tip`) serves every `data-tip` and `title` in the page; `setTip(el, text)` sets one. It waits before it shows, by two questions:

1. **How much does the tip matter?** Is it the only way to read something, or does it just name a control?
2. **How obvious is the item without it?** An obvious item, such as a visible label, a universal icon (close, zoom, trash, play, gear, sun or moon), or a resize grip, moves one tier slower.

| Tier | Delay | For | Examples |
| --- | --- | --- | --- |
| fast | 100 ms | The tip carries information the item does not show. | Lint messages in the gutter and on a line, run-dialog file errors, truncated sample values, the structure info icon, key icons, the table-match note, who else is connected |
| medium | 200 ms | The default: an icon whose meaning is not obvious, or a status whose detail is only in the tip. | Collapse and expand folders, refresh tables, group tables, structure buttons, show query, row JSON, the right-pane toggle, diagram fit and full screen, run status marks, the lint status, the table-cache and describe-cache tips |
| slow | 400 ms | The item already says what it is; the tip adds a little (a shortcut, a unit, a file type). | Both Run buttons (the action, beside a shortcut-only label), Stop (Esc), the table search hint, the capacity pill (RCU, WCU), theme, new file (.dql), clear results, the inline run button, the Running mark |

**No tooltip** when it would only repeat what the item shows or what everyone reads the same way: the endpoint chip (its popover has the details), buttons with a visible label (Run file), Close (×) buttons, zoom in and out, settings (gear), resize grips, and the time and row-count pills. Give these an `aria-label` where the visible text is not the whole name.

- `TIP_TIERS` in `app.js` lists the fast and slow selectors; anything else is medium. Put a new tip in a tier by the questions above, or set `data-tip-delay="fast|medium|slow"` on the element.
- Moving straight from one tip to another (within 300 ms of the last one hiding) shows the next at once. Leaving before the delay shows nothing.
- Tips with their own code use `tipDelay()` too (the editor Run tip, the presence tip). The CSS-only cache tips wait 200 ms with the `tip-delay` animation.

### Script components

`context-menu.js` (right-click menus), `run-tree.js` (the run dialog's file tree), `keys-editor.js` (the table browser's Keys box: a CodeMirror editor that colors keys, not DQL), `describe-card.js`, `note-card.js`, and `statement-text.js` each export functions that build or describe one piece of UI. A new piece of UI with its own state goes in its own module, with a spec under `playwright/tests/components/`.

### Icons

Follow the Material icons rule (`.cursor/rules/material-icons.rule.mdc`): one outlined 24px SVG per icon in `static/vendor/icons/`, referenced with `<use>` or `materialIcon(name)`. No hand-drawn paths and no icon fonts.

## States

- **Hover** changes the fill. **Focus** (`:focus-visible`) adds a 2px `--teal` ring, 2px outside the control. They must look different, so a keyboard user can tell where they are.
- **Disabled** is 60% opacity and `cursor: default`. Say why in a tooltip when it isn't obvious.
- **Running** shows progress next to what is running (the gutter mark, the progress bar by the Results title), not in a blocking overlay.
- **Empty** states say what to do next ("No file open"), in `--muted` text.
- **Errors** appear where the problem is: under the statement, in the lint pane, in the dialog that failed.

## Words

- Sentence case everywhere: "Clear table cache", not "Clear Table Cache".
- Name things as the user sees them: "table", "file", "query". No internal names.
- Counts are exact: "9 rows", "1 result", "18 warnings".
- Describe a check or an action plainly. Don't mention reading personal details.

## Accessibility

- Every icon-only control has an `aria-label`.
- Tabs, menus, lists, and dialogs use their ARIA roles (`tablist`, `menu`, `listbox`, `dialog`), as `index.html` does today.
- Text meets 4.5:1 contrast against its background in both themes.
- Respect `prefers-reduced-motion`: no animation carries meaning.

## Checking a change

1. Add or extend a Playwright spec under `playwright/tests/` (a component spec for a component).
2. Run `just web-test`. It runs every spec in light and dark.
3. Look at the screenshots in `.playwright-artifacts/`. `design-review.spec.js` writes `design-<screen>-<theme>.png` for every main screen.

## Design review, November 2026

Reviewed from the `design-review.spec.js` screenshots, in light and dark, at 1440×900 and 820×640.

### Fixed in this review

- **Buttons differed per dialog.** Five dialogs styled their buttons by id, with two heights (30 and 32px), two radii, and both outlined and filled Cancel buttons. They now share `.btn`.
- **Close buttons were written three times**, with three different hover colors. They now share `.icon-btn`.
- **Dialog resets, backdrops, cards, and titles were repeated per dialog.** They now share `.dialog`, `.dialog-card`, and `.dialog-title`. `app.css` is about 300 lines shorter.
- **Focus looked the same as hover** on dialog close buttons (`outline: none` plus the hover fill). Buttons and icon buttons now get the shared focus ring.
- **The run dialog's Stop was outlined** while the editor's Stop is filled red. Both are filled now.
- **The font came from Google Fonts**, so the app made an outside request and fell back to a system font offline. Noto Sans is now bundled.
- **Disabled pager buttons looked enabled.** Previous and Next on a single page had no disabled style. They are now dimmed.

### Open findings

| Finding | Where | Suggestion |
| --- | --- | --- |
| The run dialog fills the window even for one file. | Run file | Use a card for a single file; keep the sheet for folders. |
| The Run button shows only a play icon and "Ctrl + Enter". | Editor header | Add the word "Run" for first-time users. |
| Markdown lines run the full window width (over 150 characters at 1440px). | Document view | Cap the text measure near 90 characters; let tables and diagrams stay wide. |
| `app.js` is about 5,850 lines and holds every screen. | `static/app.js` | Split it into modules: [plan](../rust-docs/plans/todo/split_web_app_js.md). |
| Toolbar controls are still styled by id and by location. | `app.css` | Move them to a `.btn-sm` component. |
| About 37 `:focus-visible` rules each set their own focus style. | `app.css` | Move them to the shared ring as each screen is touched. |
| Font sizes include half steps (12.5, 13.5, 11.5px) and eight radii are in use. | `app.css` | Fold them into the scales above. |
| The sheet dialogs (`.schema-pop-card`, `.row-pop-card`) repeat the same card and header. | `app.css` | Share a `.dialog-sheet-card` and header. |
