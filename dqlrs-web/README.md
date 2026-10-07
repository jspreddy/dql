# DQLRS Web

A compact local UI for `.dql` files. The left side is the folder tree of the directory you start in. Right-click a file or folder there to create, duplicate, rename, move, or delete it. Drag an entry onto a folder to move it, or double-click its name to rename it. The top bar switches between a query editor and a table browser.

```bash
./dqlrs-web/run.sh --dir /path/to/queries
```

DynamoDB Local on `localhost:8000` is the default. Set `DQLRS_BIN` if `dqlrs` is not on `PATH` or under `rust-impl/target`. `--aws` talks to AWS instead of Local.

In the editor, **Run selection** runs the highlighted text. With no highlight, it runs the query at the cursor. A border marks that range. Ctrl+Enter (⌘Enter on a Mac) does the same while the editor is focused. Results open underneath. The editor is CodeMirror with a DQL mode: add words in `static/dql-tokens.js` to extend highlighting. Even reads get a light background. Writes (`insert`, `update`, `delete`, `drop`, `create`, `alter`, `load`) get a light red background, and even writes use a darker red so neighboring writes stay separate. A blank line ends a highlight and keeps the previous color. A comment that touches a query, above or below, uses that query's color. `DROP` and `DELETE` are red. **Minimal write highlight** draws writes as a red gutter bar instead of a red background. **Even/odd highlight** turns the alternating bands on or off. Both choices are saved in the browser.

**Tables** lists `ls` output. The search box uses the same glob patterns as `ls` (`nb_*`, `*foo*`, `foo-*`). Opening a table shows 50 rows at a time.
