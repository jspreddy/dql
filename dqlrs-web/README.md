# DQLRS Web

A compact local UI for `.dql` files. The left side is the folder tree of the directory you start in. The top bar switches between a query editor and a table browser.

```bash
./dqlrs-web/run.sh --dir /path/to/queries
```

DynamoDB Local on `localhost:8000` is the default. Set `DQLRS_BIN` if `dqlrs` is not on `PATH` or under `rust-impl/target`. `--aws` talks to AWS instead of Local.

In the editor, highlight one or more statements and use **Run selection**. With no highlight, the whole file runs. Results open underneath. The editor is CodeMirror with a DQL mode: add words in `static/dql-tokens.js` to extend highlighting. Even queries get a light background. Writes (`insert`, `update`, `delete`, `drop`, `create`, `alter`, `load`) get a light red background, and `DROP` and `DELETE` are red.

**Tables** lists `ls` output. The search box uses the same glob patterns as `ls` (`nb_*`, `*foo*`, `foo-*`). Opening a table shows 50 rows at a time.
