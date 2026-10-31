# PRD: a UI framework for the web client

**Status:** proposal. Decision is not to do this for performance. Do not start a rewrite from this document.

## Problem

`dqlrs-web/static/app.js` builds the page by hand. It creates DOM nodes, and several updates throw a whole subtree away and build it again:

- `refreshTree` replaces the file tree after `POST` reads of the tree.
- `showResults` and `renderResults` rebuild result cards.
- `dataTable` builds a fresh table for each 50-row page.
- `renderPresence` already skips the rebuild when the client list is unchanged.

That style is easy to get wrong, and a keyed list in React, Vue, or Svelte would update fewer nodes when one row changes. It is not why the page feels slow.

The slow calls stay outside the renderer:

- Once a second, `watchOpenFile` posts `POST /api/file/read` and downloads the whole `.dql` file, then compares `revision`. The revision is a SHA-256 of that text, computed on the server after the read.
- Lint posts the whole editor buffer (`POST /api/lint`) on a 400 ms timer.
- Each finished statement is its own `POST /api/query-results`, then the page loads the history again.
- `loadTables` posts `POST /api/tables` 250 ms after each change in the table search box.
- `table_rows` in the Rust server runs `SCAN * FROM name LIMIT 50 * (page + 1)` and returns one page. Page 5 scans 300 items to show 50.

CodeMirror is the large script. `codemirror-view.js` is about 494 KB. `app.js` is about 147 KB. The editor is already a library. Replacing the surrounding page with another library does not shrink that.

## Who this is for

Someone deciding whether a framework rewrite is the performance project. It is not a request to migrate.

## What a rewrite would take

The server would need a build the page does not have today. `index.html` loads CSS and an import map of plain files. `serve_static` returns those files. React, Vue, or Svelte want a compile step, a `node_modules` tree, and a bundle. That bundle has to be what `dqlrs --web` serves, and Playwright would have to keep finding the same buttons and tables.

Svelte emits the smallest runtime and is the closest to the current "update this node" style. Vue is in between. React adds a runtime and a render pass that is easy to make slower than `createElement` if result cards re-render from the top. None of the three can paint CodeMirror. The editor stays an island either way.

Playwright covers the editor, the file watch, and the table browser by clicking the current DOM. A rewrite has to keep those tests green or replace them. That is most of the cost. The realistic saving is fewer DOM nodes when a 50-row table or the file tree is redrawn. That is milliseconds next to a DynamoDB scan.

## Requirements if this is ever reopened

1. Measure a named screen first (open file, run a script, page a table) and show the framework build is faster on that screen before the rest of the page moves.
2. Keep `dqlrs --web` able to serve the built page with no extra dev server.
3. Keep CodeMirror as the editor. Do not reimplement the buffer in the framework.
4. Keep the Playwright flows for the editor, file watch, and table list passing.
5. Do not change the HTTP API as part of the rewrite.

## Non-goals

- Adopting React, Vue, or Svelte as the way to fix the one-second file read, lint posts, per-statement result saves, or table scans.
- A visual redesign.
- Replacing CodeMirror.
- Shipping a framework from a CDN. The page is a local static tree.

## Decision

Do not rewrite the web client onto a framework for performance. If the hand-written DOM becomes hard to change, revisit Svelte only, and only behind a measurement on one screen. Until then, the faster work is smaller HTTP bodies and fewer round trips, which this proposal does not include.

## Done when

- This document stays the record of the decision.
- No framework dependency is added to `dqlrs-web` under this proposal.
