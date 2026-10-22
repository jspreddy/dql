---
name: work-metrics
description: Measure rewrite work from commit subjects, pull request merges, and a Cursor token-spend CSV. Use when the user asks for work metrics, hours worked, token spend, commits against usage, or a timeline of the rewrite. Ask for the CSV before running. The report bin defaults to day.
---

# Work metrics

Measure the rewrite from commit subjects, author dates, pull request titles and merge times, and a Cursor usage export. Use Python, pandas, and Plotly. Do not hand-build SVG.

Do not read diffs, commit bodies, or file contents. `git log` subjects, authors, and author dates are enough, plus `gh pr list` for merged pull requests.

## Before running

1. If the user has not attached a Cursor token-spend CSV for the timeframe, ask them to attach that export and stop. Do not estimate tokens, do not invent a Cost, and do not download the Cursor dashboard export.

The report bin defaults to day. Pass `--bin month` when the user asks for a month rollup. There is no hour bin.

The attached file is the timeframe. A row counts toward token spend only under the rule below. A numeric Cost is the only dollar figure. `Included` and `Free` are not prices, and tokens are not converted into dollars.

## Run

From the repository root:

```bash
python3 .cursor/skills/work-metrics/work_metrics.py \
  --bin day \
  --csv /path/to/usage.csv \
  --out "$(mktemp -d)" \
  --html rust-docs/work-metrics.html
```

`--bin` defaults to `day`. The other value is `month`. Install pandas and Plotly with pip if they are missing. Do not add them to the project's Python package.

The script prints `out:` and `html:`. `out` holds the JSON and the graph fragments. `html` is the page to show. In this repository that page is `rust-docs/work-metrics.html`. Add a link from `rust-docs/README.md` if it is missing.

Defaults are `--base origin/v-next` and `--head origin/v-rust`. Author dates are converted to UTC before any binning. `%aI` without that conversion buckets days wrong.

## Outputs

In the temp folder:

- `commits-by-bin.json` — rewrite commits grouped by the chosen bin, each bin ordered by author time. This is the commit timeline.
- `timeline.json` and `timeline.html` — one point per bin, from the first commit or token day through the last, empty bins included. Hours worked are blue bars. Tokens are green dots, in millions. Commits are red circles sized by the count.
- `tokens-by-bin.json` and `tokens.html` — token spend for the chosen bin.
- `pulls-by-day.json` and `pulls.html` — pull requests merged, grouped by the report bin. A pull request counts when its merge commit is in `base..head`.
- `tokens-by-agent.json` and `agents.html` — tokens by cloud agent.
- `tokens-by-model.json` and `models.html` — tokens by model. The page keeps the first seven models and rolls the rest into Other.
- `hours-worked.json` and `hours.html` — hours worked, defined below.
- `hours-by-category.json` and the category pie — hours worked split by area, including other commits in this repo. Category descriptions are a bullet list under Areas of work.
- `summary.json` and `work-metrics.html` — the combined page.

Gather commits, usage, and pull requests at UTC hour grain. An hour is worked when that hour has a commit on any ref in this repo. The report then sums those hours, the commits, the tokens, and the merged pull requests into the bin. On the timeline, hours worked are blue bars, tokens are green dots, and commits are red circles. The circles sit on one horizontal line, 4 hours above the highest hours-worked bar. Their size shows the commit count.

Series colors live in `phases.json` and in `PALETTE` in the script. Neighboring series stay far apart in hue. Do not put two browns, two blues, or two greens next to each other.

## Which usage counts

Keep a usage row when either of these is true:

- Its cloud agent id starts with a prefix in `agents` in `phases.json`. Those agents are this repo, so the row stays even when that UTC hour has no commit.
- Its UTC hour has a commit on any ref in this repo, and the id is not in `other_repo_agents`. A blank cloud agent id uses this rule.

Leave a row out when its id is in `other_repo_agents`, or when the hour has no commit here and the id is not a this-repo agent.

An id that is on neither list is not clearly another repo. Treat it like a blank id: it counts only in an hour with a commit here. When you can tell where an id belongs, add it to `agents` or to `other_repo_agents`. Do not guess.

`bc-ca207e1b` is the other-repo id in the current export. It is not a cloud agent on `github.com/jspreddy/dql`.

## Hours worked

A UTC hour is worked when it has a commit on any ref in this repo. Token usage does not make an hour worked. The page sums those hours into the report bin.

Say both counts when they differ: rewrite commit hours, and other commit hours in this repo. Do not narrate the empty stretches beyond that.

## Areas of work

These hours are canon. `phases.json` is the list the script loads. Do not reclassify them, and do not give every hour on a calendar day the same area. 10 Oct contains both Rust improvements and Web.

Rust impl ended on 20 Aug, as the acceptance tests were done. October engine, `ls`, DESCRIBE, SHOW TABLES, SAVE, and REPL hours are Rust improvements. The 3 Oct pexpect hour stays acceptance tests. `2026-10-12T15` stays outside these groups: that hour is split between a SAVE change and a black-box flake. `2026-10-14T05` stays Rust improvements.

Hours below are UTC, `YYYY-MM-DDTHH`. The number of hours is the length of each list.

### Rust impl — 23 hours

Through 20 Aug. Days: 5 Jul (5), 6 Jul (2), 8 Jul (2), 10 Jul (7), 17 Jul (1), 22 Jul (1), 19 Aug (1), 20 Aug (4).

`2026-07-05T18` `T19` `T20` `T21` `T22`, `2026-07-06T21` `T22`, `2026-07-08T02` `T03`, `2026-07-10T15` `T16` `T17` `T19` `T20` `T21` `T22`, `2026-07-17T19`, `2026-07-22T17`, `2026-08-19T15`, `2026-08-20T00` `T18` `T19` `T20`.

### Rust improvements — 14 hours

Days: 4 Oct (1), 10 Oct (4), 11 Oct (3), 12 Oct (2), 13 Oct (2), 14 Oct (2).

`2026-10-04T04`, `2026-10-10T12` `T13` `T15` `T17`, `2026-10-11T03` `T13` `T16`, `2026-10-12T14` `T23`, `2026-10-13T15` `T16`, `2026-10-14T02` `T05`.

### Acceptance tests — 12 hours

Days: 21 Aug (6), 23 Aug (5), 3 Oct (1).

`2026-08-21T15` `T17` `T18` `T19` `T20` `T22`, `2026-08-23T17` `T18` `T19` `T20` `T21`, `2026-10-03T20`.

### Notebook — 10 hours

Days: 18 Sep (1), 19 Sep (5), 4 Oct (3), 5 Oct (1).

`2026-09-18T23`, `2026-09-19T01` `T04` `T13` `T16` `T20`, `2026-10-04T13` `T20` `T22`, `2026-10-05T00`.

### Web — 40 hours

Days: 7 Oct (7), 8 Oct (1), 10 Oct (7), 11 Oct (3), 12 Oct (4), 14 Oct (8), 15 Oct (6), 18 Oct (2), 19 Oct (2).

`2026-10-07T17` `T18` `T19` `T20` `T21` `T22` `T23`, `2026-10-08T01`, `2026-10-10T00` `T02` `T03` `T04` `T05` `T06` `T16`, `2026-10-11T06` `T15` `T22`, `2026-10-12T13` `T20` `T21` `T22`, `2026-10-14T13` `T14` `T15` `T17` `T18` `T19` `T20` `T21`, `2026-10-15T00` `T01` `T03` `T06` `T12` `T13`, `2026-10-18T22` `T23`, `2026-10-19T00` `T02`.

Any other rewrite commit hour is outside these groups: git graph, CI, README and screenshots, justfiles, explainers, version bumps, flake fixes, and the split hour on 12 Oct.

## Cloud agents

| Prefix | Name |
| --- | --- |
| `bc-a467b549` | Notebook plan |
| `bc-7bbeede0` | DQLRS fork |
| `bc-79cf3007` | Rust IMPL |
| `bc-7c9a145b` | DQLRS |
| `bc-8f9acc09` | Acceptance tests |
| `bc-15659210` | Rust review |
| `bc-7e6778af` | Agent configuration |
| `bc-c2ae0364` | Pending plans |
| `bc-fed04fbe` | Rewrite metrics |

An empty cloud agent id is "No cloud agent id". An id on neither list is "Other agents" and counts only in an hour with a commit in this repo.

`bc-ca207e1b` is listed under `other_repo_agents` and does not count.
