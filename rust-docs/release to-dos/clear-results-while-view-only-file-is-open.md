# Clear results does nothing while a view-only file is open

Status: open. Found on `0.7.12-alpha.1` while reviewing the Run folder fix from #295 (`5e8e924`).

## What goes wrong

Open a markdown, CSV, JSON, or HTML file. Right-click a folder, or another `.dql` file, and choose **Clear results**. The menu item is there. Nothing is cleared, and there is no error.

## Why

A view-only file disables the editor Run button in `showOpenFile` (`dqlrs-web/static/app.js`). **Run folder** and **Run file** used to bail out on that same flag. #295 stopped `openRunDialog` from doing that. `clearFileResults` still does:

```js
if (!path || runButton.disabled) return;
```

The context menu still adds **Clear results** for every folder, and for a `.dql` file, in `menuItems`. The click reaches `clearFileResults` and returns before `POST /api/query-results/clear`.

The server side is already folder-aware. `query_cache::clear_at` clears every `.dql` file under the chosen folder. The request never gets there.

A run already in progress also disables the Run button, via `setRunBusy`. That case should keep refusing a clear. A view-only open file should not.

## What to change

Gate the clear on “a run is in progress”, the same way `openRunDialog` now checks that the stop button is showing, not on `runButton.disabled`.

Cover it in `dqlrs-web/playwright/tests/files.spec.js` next to “opens Run folder while a markdown, csv, or json file is open”: with one of those files open, **Clear results** on a folder that has saved results must call the clear and drop those results.
