# PRD: web workers for the web client

**Status:** proposal. Decision is not to do this for performance. Do not add a worker from this document.

## Problem

The page does its work on the main thread: CodeMirror, DOM updates, and `fetch`. A Web Worker can run JavaScript off that thread. It cannot touch the DOM, and it cannot touch the editor.

What the main thread actually does that is not painting:

- `file-patch.js` `unifiedDiff` builds a line diff of the unsaved editor text against the copy on disk. The comparison is a table of `Uint32Array` rows. If the line counts multiply to more than 250,000, it skips the comparison and marks every line removed and added. The diff runs when a watched file changes and the editor is dirty, not on each keystroke.
- `dql-tokens.js` `tokenDql` and `highlightParts` classify words for the editor and for painted query text. CodeMirror already tokenizes the visible buffer incrementally on the main thread.
- `jsonParts` in `app.js` splits one row's JSON with a regular expression when a row popup opens.
- `readNdjson` parses each run event with `JSON.parse` and then updates the progress bar and the gutter. Those updates have to happen on the main thread.

`fetch` itself does not block painting. The thread waits again when the JSON is turned into DOM. A worker can parse bytes, then the main thread still builds the table.

The expensive waits are not on this thread. File watch, lint, query runs, and table scans are HTTP to the Rust server, and row paging is a DynamoDB `SCAN` whose limit grows with the page number.

## Who this is for

Someone deciding whether a worker thread is the performance project. It is not a request to add one.

## What a worker would take

`new Worker(url, { type: "module" })` can load a static file the server already serves, so a bundler is not required. Strings and result objects are copied across the boundary (or transferred, which the sender can no longer use). For a query file of a few kilobytes, that copy is the whole cost of the diff the worker was meant to speed up.

A worker that tokenizes for CodeMirror has to duplicate the stream parser or accept stale highlights. CodeMirror's stream mode is built to run inside the editor on the main thread. Pulling it out loses the incremental update.

A worker that parses the run stream still posts every `progress` and `result` event back so the page can paint. The messages are the same work `readNdjson` does now, plus a hop.

Sharing the file text with `SharedArrayBuffer` needs cross-origin isolation headers. The local server does not send those, and the diff is not hot enough to justify them.

## Requirements if this is ever reopened

1. Name one main-thread task that shows up in a profile while the page is in use, and show the worker version is faster including the copy.
2. Leave CodeMirror and every DOM write on the main thread.
3. Do not require `SharedArrayBuffer` or new response headers.
4. Keep a worker failure from blocking save, run, or file watch. The main thread must still be able to do the job.
5. Do not change query results or the HTTP API.

## Non-goals

- Moving `fetch`, file watch, lint, or table scans into a worker. Those wait on the server.
- Running the editor off the main thread.
- Parsing every keystroke in a worker.
- Using a worker as a substitute for a smaller file-read or a smaller row scan.

## Decision

Do not add a web worker for performance. The one algorithm that is actually heavy, the unsaved-edit diff, already refuses the expensive comparison, and it runs rarely. Copying the file into a worker would not make the page feel faster. If a profile later shows a multi-megabyte diff or a multi-megabyte JSON parse blocking input, a worker around that one function can be reconsidered. The worker must be optional and must not own the DOM.

## Done when

- This document stays the record of the decision.
- `dqlrs-web/static` gains no `Worker` under this proposal.
