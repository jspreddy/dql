# Web client performance proposals

Plans only. No client or server code in this set.

The page is `dqlrs-web/static`: one hand-written script (`app.js`, about 147 KB), a few small modules, and CodeMirror from an import map. The Rust server in `dqlrs --web` serves those files as-is. There is no bundler.

The waits a person feels are HTTP and DynamoDB, then painting the result. A UI framework and a web worker do not move that work.

| Proposal | Document | Decision |
| --- | --- | --- |
| New UI framework | [`prd-framework.md`](prd-framework.md) | Do not rewrite the page for speed |
| Web workers | [`prd-workers.md`](prd-workers.md) | Do not move client work off the main thread for speed |

Wasm was looked at separately and is not one of these proposals. Queries, lint, scans, and cache encryption already run in the Rust server. Compiling more Rust to Wasm for the browser would still wait on the same HTTP calls.
