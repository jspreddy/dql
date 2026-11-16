# TODO: Split `dqlrs-web/static/app.js` into modules

**Priority:** Medium  
**Smell / SOLID:** God file, SRP, shared mutable state  
**Area:** `dqlrs-web/static/`

Status: planned. Nothing here is started.

## Problem

`app.js` is about 5,850 lines and 291 top-level functions. It holds every screen of the web
app: the file tree, the editor wiring, results, the table browser, dialogs, live file sync, and
the server API. Everything shares one `state` object (about 40 fields, read or written in about
300 places) and reaches into the page with `document.querySelector` about 180 times.

- A change to one screen means reading the whole file.
- Nothing in `app.js` can be tested without booting the whole app in a browser.
- The pieces that were moved out earlier (`dialogs.js`, `context-menu.js`, `run-tree.js`,
  `statement-text.js`, `describe-card.js`, `note-card.js`, `file-patch.js`) are small, have
  their own tests, and have not needed attention since.

## Goal

`app.js` becomes the entry point: it imports the modules, creates them, and wires them together.
Target: under 800 lines, with no rendering code of its own.

## Rules for every extraction

1. **No behavior change.** One module per pull request. The full `just web-test` suite passes
   before and after, in light and dark.
2. **No build step.** Plain ES modules, loaded by the browser, as today.
3. **A module does not import `app.js`.** It gets what it needs as arguments: the elements it
   owns, the `api` function, and callbacks for anything outside itself (`onOpenFile`,
   `onRun`). This is how `dialogs.js` and `context-menu.js` already work.
4. **A module owns its state.** Fields move out of the shared `state` object into the module
   that uses them. A field two modules need stays in `app.js` and is passed in, or goes behind
   a small getter.
5. **Pure logic is split from DOM code** where a module has both, so the logic gets a fast
   `node --test` test in `dqlrs-web/tests/` (as `describe-card.js` and `file-patch.js` do).
6. **Each module gets a test:** a component spec in `playwright/tests/components/` for DOM
   modules, a `node --test` file for pure ones.
7. **Follow `dqlrs-web/DESIGN.md`** for anything visual, and take the chance to move the
   module's one-off focus and button styles to the shared components.

## Modules

Line counts are approximate, from the function positions in `app.js` today.

| # | Module | Takes from `app.js` | ~Lines | Depends on |
| --- | --- | --- | --- | --- |
| 1 | `api.js` | `requestPath`, `apiPath`, `tokenError`, `fetchServer`, `api`, `readNdjson`, server-away detection | 150 | nothing |
| 2 | `markdown.js` | `renderMarkdown`, tables, inline text, `safeHref`, `escapeHtml` | 140 | nothing (pure) |
| 3 | `json-view.js` | `highlightedJson`, `jsonParts` | 40 | nothing (pure) |
| 4 | `explain.js` | `parseExplainPlan`, `parseExplainFields`, `explainRows`, `explainPlan` | 170 | pure parsing + one renderer |
| 5 | `tooltips.js` | `setTip`, `claimTip`, lint and run tips, presence tip placement | 200 | nothing |
| 6 | `resize.js` | results-pane and side-nav drag, limits, saved sizes | 200 | nothing |
| 7 | `settings.js` | theme, highlight options, file-type filters, settings popover, cache-clear buttons | 250 | `api.js` |
| 8 | `mermaid-view.js` | load, theme variables, render, expand, full screen, zoom, pan, call-chain highlight | 430 | nothing |
| 9 | `data-table.js` | `dataTable`, key columns and icons, `tableColumns`, row peek button | 180 | nothing |
| 10 | `row-dialog.js` | `openRowJson`, key chips, ordered row | 150 | `json-view.js` |
| 11 | `schema-dialog.js` | `openSchema`, definition and result sections, type chips, presence bars | 370 | `data-table.js` types |
| 12 | `result-cards.js` | `renderResults`, `resultCard`, stat chips, query toggle, describe card, summary table, help card, notes | 600 | `data-table.js`, `explain.js`, `describe-card.js` |
| 13 | `results-store.js` | per-file results, pinning, counts, result/history/lint panes | 200 | `result-cards.js` |
| 14 | `title-search.js` | search box, result list, keyboard, `openSearchHit` | 220 | `api.js` |
| 15 | `environment.js` | endpoint button, environment popover, expiry | 110 | `api.js` |
| 16 | `presence.js` | connected clients, tip | 100 | `tooltips.js` |
| 17 | `complete.js` | catalog cache, `siteAt`, suggestions, the suggestion list | 220 | `api.js`, editor |
| 18 | `lint.js` | schedule, apply, lint pane, lint status | 140 | `api.js`, editor |
| 19 | `file-tree.js` | tree render, expand and collapse, drag and drop, rename, file operations, menu items | 550 | `api.js`, `dialogs.js`, `context-menu.js` |
| 20 | `documents.js` | `showDocument`, HTML frame, workspace links, CSV and JSON file rows and paging | 220 | `markdown.js`, `mermaid-view.js`, `data-table.js` |
| 21 | `file-sync.js` | save queue, event stream, external changes, adopt, patch dialog | 420 | `api.js`, `file-patch.js`, editor |
| 22 | `run.js` | run selection, run file, run dialog, progress, gutter marks | 420 | `api.js`, `run-tree.js`, `results-store.js`, editor |
| 23 | `tables-nav.js` | table list, search, grouping, cache age, refresh, table browser rows and paging | 330 | `api.js`, `data-table.js`, `schema-dialog.js` |
| 24 | `editor.js` | the CodeMirror view, extensions, theme, `setEditorText`, `editorText` | 150 | `dql-mode.js` |

What stays in `app.js`: `boot`, `setMode`, `openFile`, the remaining shared state, and the
wiring between modules.

## Order

Leaves first, so each step is small and the shared state shrinks as it goes.

1. **Pure code (1 to 4).** No DOM state, easy to test, no risk. Proves the pattern.
2. **Independent widgets (5 to 8).** They touch their own elements only.
3. **Tables and dialogs (9 to 11).** `data-table.js` first: query results, the table browser,
   and data files all use it. This is also where a custom element (`<dql-table>`, without
   shadow DOM) can be tried; see "Web components" below.
4. **Results (12, 13).** The largest block of rendering code.
5. **Header and side features (14 to 18).**
6. **The core (19 to 24).** The file tree, file sync, and run all read and write the open
   file, the editor, and each other. Extract `editor.js` first here, then define the small
   interface the rest share (current path, editor text, open a file, show results) before
   moving them.

Stop after any step: every step leaves the app working and `app.js` smaller.

## Shared state

Before step 6, sort the `state` fields by owner:

| Owner | Fields |
| --- | --- |
| `file-tree.js` | `tree`, `expanded`, `dragPath` |
| `file-sync.js` | `saveTimer`, `suppressSave`, `disk`, `fileEpoch`, `patches`, `patchIndex`, `saveFlight`, `saveChain`, `applyingDisk`, `adopting`, `adoptQueued`, `watchSeq`, `notedExternal`, `eventsLive` |
| `tables-nav.js` | `table`, `entries`, `page`, `hasMore`, `tablesRequest`, `listGeneration`, `tableListNote`, `cachePoll`, `cacheStatusRequest`, `listedTables`, `browserSchema` |
| `documents.js` | `filePage`, `fileHasMore`, `fileSchema` |
| `complete.js` | `catalog` and the `complete*` variables |
| `lint.js`, `title-search.js` | `lintTimer`, `searchTimer` |
| `app.js` (shared) | `mode`, `path`, `environment`, `booted`, `clientId`, `schemaKind` |

## Web components

Not part of this plan's default. Styling-only pieces (buttons, dialog cards) stay CSS classes.
When `data-table.js` is extracted (step 3), build it once as a custom element without shadow
DOM and compare it with the factory-function version: lines of code, cleanup of listeners, and
how the three callers read. Use custom elements for later modules only if that comparison
clearly favors them.

## Risks

- **Hidden coupling through `state` and `document.querySelector`.** A moved function may read
  a field or an element another feature also changes. The Playwright suite (about 250 tests)
  is the guard; run it for every extraction.
- **Load order.** `app.js` runs top to bottom and some code depends on earlier side effects
  (elements looked up at load, listeners attached in order). Modules export a `create…`
  function that `app.js` calls, so the order stays explicit.
- **Flaky tests hide regressions.** Fix known flakes first (for example the `ST lists tables`
  test in `tables.spec.js`, which can read the previous result card).

## Done when

- `app.js` is under 800 lines and renders nothing itself.
- Every module in the table has a test.
- `just verify` passes.
