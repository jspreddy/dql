# DQLRS Web

A compact local UI for `.dql` files. The left side is the folder tree of the directory you start in. Right-click a file or folder there to create, duplicate, rename, move, or delete it. Drag an entry onto a folder to move it, or double-click its name to rename it. The top bar switches between a query editor and a table browser.

```bash
just web
./dqlrs-web/run.sh --dir /path/to/queries
```

`just web` serves `examples/` and starts DynamoDB Local when port 8000 is down. Extra arguments are passed through: `just web -- --port 9000`. `just web -- --aws` talks to AWS and does not start Local. Set `DQLRS_BIN` if `dqlrs` is not on `PATH` or under `rust-impl/target`.

In the editor, **Run selection** runs the highlighted text. With no highlight, it runs the query at the cursor. A light background marks that range, as one block when it covers more than one line, with a blue border 5px out from the text. Ctrl+Enter (⌘Enter on a Mac) does the same while the editor is focused. Results open underneath. An explain plan lists each operation, the table it touches, and the index, key condition, and filter on their own lines. The JSON toggle beside the Explain title shows that same plan as formatted JSON. Drag the handle along the top of the Results bar to change how tall that pane is; a double-click returns it to the usual height. While a query runs, a progress bar sits next to the Results title. The gutter shows a blue mark on that query, then a green check or a red cross. The editor is CodeMirror with a DQL mode: add words in `static/dql-tokens.js` to extend highlighting. Even reads get a light background. Writes (`insert`, `update`, `delete`, `drop`, `create`, `alter`, `load`) get a light red background, and even writes use a darker red so neighboring writes stay separate. A blank line ends a highlight and keeps the previous color. A comment that touches a query, above or below, uses that query's color. `DROP` and `DELETE` are red. **Minimal write highlight** draws writes as a red gutter bar instead of a red background. **Even/odd highlight** turns the alternating bands on or off. Both choices are saved in the browser.

**Tables** lists `ls` output. The search box uses the same glob patterns as `ls` (`nb_*`, `*foo*`, `foo-*`). Opening a table shows 50 rows at a time.

Playwright coverage lives in `playwright/`. `just web-test` runs the suite in system Chrome against a temporary copy of `playwright/fixtures` and DynamoDB Local. Pass Playwright arguments after `--`, for example `just web-test -- tests/editor.spec.js`. From `playwright/`, `npm test` is the same suite.
