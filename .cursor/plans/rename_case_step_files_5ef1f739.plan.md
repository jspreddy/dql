---
name: Rename case step files
overview: Prefix only harness-consumed case files (plus 00-README.md) so each case folder sorts in run order. Rename the asserted-commands file from input.dql to 30-test.dql. Fixtures the harness does not load, such as seed.json, stay unprefixed.
todos:
  - id: rename-files
    content: git mv harness-consumed case files to 00-/10-/20-/30-test-/40-/50- prefixes; leave seed.json as-is
    status: pending
  - id: harness-constants
    content: Define CASE_* names in lib.sh (CASE_TEST=30-test.dql); update discovery, verbose listing, and all case_dir reads in run.sh; rename the Input verbose step to Test
    status: pending
  - id: docs-generator
    content: Update README.md, ordering.md, generate_records.py; call the asserted-commands file 30-test.dql (not input)
    status: pending
  - id: verify
    content: Confirm folder sort order and that the harness still finds all cases via 30-test.dql
    status: pending
isProject: false
---

# Prefix harness case files to match step order

Case **directories** stay as they are (`000-cli-version`, `010-create-…`). Number-prefix only files the harness reads as steps (and `00-README.md` so docs sit at the top). Files the harness never opens stay unprefixed.

Rename the asserted-commands step from **input** to **test**: that file is what is under test, not a generic input. The harness flow is setup → test → assert → teardown.

## What gets a prefix

Prefix a file only if [`run.sh`](black-box-tests/harness/run.sh) / [`lib.sh`](black-box-tests/harness/lib.sh) opens it from the case directory (`mode`, `setup.dql`, `input.dql` today, `teardown.dql`, `expected.*`). `00-README.md` is the one exception: not a harness input, but numbered so it sorts first.

**Do not prefix** fixtures referenced only from DQL, such as `seed.json`. The harness does not load it; `setup.dql` does via `LOAD`. Shared [`fixtures/pk-sk-records/seed.json`](black-box-tests/fixtures/pk-sk-records/seed.json) is also unchanged.

## Target names

| New name | Required | Today |
| --- | --- | --- |
| `00-README.md` | no | `README.md` |
| `10-mode` | no | `mode` (none exist yet) |
| `20-setup.dql` | no | `setup.dql` |
| `30-test.dql` | yes | `input.dql` |
| `40-expected.json` / `40-expected.stdout` | one of | current `expected.*` |
| `40-expected.stderr` / `40-expected.exit` | no | none exist yet |
| `50-teardown.dql` | no | `teardown.dql` |
| `seed.json` | no | unchanged (only `110-load-json-into-table`) |

Same prefix (`40-`) for all oracles so alternatives stay adjacent. Steps are consecutive; there is no reserved slot for seed.

A typical case becomes:

```text
00-README.md
20-setup.dql
30-test.dql
40-expected.json
50-teardown.dql
```

`110-load-json-into-table` also keeps `seed.json` (sorts after the numbered files). `LOAD 'seed.json'` in [`110-load-json-into-table/setup.dql`](black-box-tests/cases/110-load-json-into-table/setup.dql) stays as-is.

## 1. Rename files with `git mv`

In every `black-box-tests/cases/<NNN>-*/` directory that has `input.dql`, rename the harness files that exist (skip missing optionals), including `input.dql` → `30-test.dql`. Leave `seed.json` in place.

## 2. Centralize names in the harness

Add constants once in [`black-box-tests/harness/lib.sh`](black-box-tests/harness/lib.sh) (e.g. `CASE_TEST=30-test.dql`, `CASE_SETUP=20-setup.dql`, …) and use them for:

- Case discovery (`list_case_dirs` currently requires `input.dql`; require `30-test.dql`)
- `harness_verbose_test_files` (iterate prefixed harness names in step order; if `seed.json` exists, list it unprefixed as a fixture, not a step)
- All `"$case_dir/…"` reads in [`black-box-tests/harness/run.sh`](black-box-tests/harness/run.sh): `mode`, `setup.dql`, `input.dql` → `30-test.dql`, `teardown.dql`, `expected.json` / `stdout` / `stderr` / `exit`

Rename the verbose step label from `"Input"` to `"Test"` (the `harness_verbose_step` call that currently prints `Input` / `input.dql`).

Keep **work-dir** copies unprefixed (`$work/setup.dql`, `$work/test.dql`, …). Those are temp files, not what you browse. Verbose logs that describe the *case* file should show the prefixed name.

Update the `run.sh` usage string (`teardown.dql`) and the “no cases with input.dql” error to `30-test.dql`.

## 3. Other code and docs

- [`black-box-tests/fixtures/pk-sk-records/generate_records.py`](black-box-tests/fixtures/pk-sk-records/generate_records.py): `SELECT_EXPECTED_PATH` currently ends in `expected.json`; point it at `40-expected.json`.
- [`black-box-tests/README.md`](black-box-tests/README.md): case-file table, “must contain”, and “Adding a case” steps. Call the asserted-commands file `30-test.dql` (not input). Keep `seed.json` unprefixed; note it is a DQL fixture, not a harness step.
- [`black-box-tests/cases/ordering.md`](black-box-tests/cases/ordering.md): replace `input.dql` with `30-test.dql` (e.g. “`SELECT` in `30-test.dql`”). Leave `LOAD seed.json` as `seed.json`.

Do not edit the historical [`.cursor/plans/black_box_acceptance_tests.plan.md`](.cursor/plans/black_box_acceptance_tests.plan.md). [`fixtures/README.md`](black-box-tests/fixtures/README.md) can stay as-is (`seed.json` in a case directory remains the unprefixed name).

## 4. Check

- `ls` a couple of cases and confirm sort order (`00-README`, then numbered harness files, then `seed.json` if present).
- `./black-box-tests/harness/run.sh` (or a numeric group) still discovers every case via `30-test.dql`.
