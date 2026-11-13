# Screenshots

Product shots used by the top-level [`README.md`](../README.md). Every shot
uses the cosmos sample from
[`dqlrs-web/examples/cosmos/`](../dqlrs-web/examples/cosmos/) against DynamoDB
Local.

| File | Shows |
| --- | --- |
| [`banner.png`](banner.png) | README banner: `dqlrs web --init-examples`, the three surfaces, MCP, and security, in the web UI's dark colors |
| [`cli/repl.png`](cli/repl.png) | REPL: `opt format rich`, then Sol's planets as a table |
| [`cli/cosmos-planets.png`](cli/cosmos-planets.png) | REPL: default `smart` format, expanded rows with nested maps |
| [`web/query.png`](web/query.png) | `dqlrs web` query editor with a result (dark theme) |
| [`web/explain.png`](web/explain.png) | `dqlrs web` explain plan card (dark theme) |
| [`web/tables.png`](web/tables.png) | `dqlrs web` table browser (dark theme) |
| [`notebook/cosmos.png`](notebook/cosmos.png) | `dqlrs notebook`: a DQL (Rust) cell and its table (JupyterLab Dark) |
| [`notebook/getting-started.png`](notebook/getting-started.png) | `dqlrs notebook`: the starter notebook it writes (JupyterLab Dark) |
| [`notebook/launcher.png`](notebook/launcher.png) | `dqlrs notebook`: launcher with the DQL (Rust) kernel (JupyterLab Dark) |

## Recapture

Seed the table first (`file` reads one path per call):

```bash
just dynamo
dqlrs -H localhost -p 8000 -c "file dqlrs-web/examples/cosmos/00-table.dql"
dqlrs -H localhost -p 8000 -c "file dqlrs-web/examples/cosmos/10-bodies.dql"
```

**Web.** The Playwright spec
[`readme-screenshots.spec.js`](../dqlrs-web/playwright/tests/readme-screenshots.spec.js)
seeds the table itself and writes three 1440×900 shots per theme to
`.playwright-artifacts/`. The README uses the dark ones (`readme-web-*-dark.png`
from the `dark` project); the light ones are there if you need them:

```bash
just web-test -- tests/readme-screenshots.spec.js --project dark
for n in query explain tables; do cp .playwright-artifacts/readme-web-$n-dark.png screenshots/web/$n.png; done
cp .playwright-artifacts/readme-web-query-dark.png screenshots/banner/web.png
```

**CLI.** Run `dqlrs -H localhost -p 8000` in a 110×34 terminal with a black
background. For `repl.png`, type `opt format rich`, then the Sol planets query
from `20-queries.dql`. For `cosmos-planets.png`, start a new session and run the
terrestrial filter query. The current shots were captured from tmux
(`capture-pane -e -N`) and rendered with xterm.js.

**Notebook.** From an empty folder that holds only `cosmos.ipynb` (a heading and
the Sol planets query), run `dqlrs notebook --local`. Open
`getting-started-dqlrs.ipynb` and `cosmos.ipynb`, run the query cell, and
capture Lab at 1440×900 with the news toast dismissed. Switch to
**Settings → Theme → JupyterLab Dark** first; start Lab with a fresh
`JUPYTERLAB_SETTINGS_DIR` so an earlier theme choice does not carry over.

**Banner.** [`banner/banner.html`](banner/banner.html) is the source. It uses
the dark tokens from `dqlrs-web/static/theme.css` and the vendored Material
icons. Its panels are dark captures: `banner/web.png` is
`readme-web-query-dark.png` from the web spec (dark project), `banner/tui.png`
is `cli/repl.png`, and `banner/notebook.png` is the cosmos notebook in
JupyterLab Dark. Render the `#banner` element at 1280×640 with
`deviceScaleFactor: 2` and Chromium's `--allow-file-access-from-files` (the
icons load from a sibling folder), and save it as `screenshots/banner.png`.
