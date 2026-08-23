---
name: Rich verbose transcripts
overview: Restore bash-harness-style per-step DQL/CLI transcripts when `-v` is set, using Rich for the same color roles, without changing the 18 test functions.
todos:
  - id: dep-rich
    content: Add rich to black-box-tests/pyproject.toml and uv.lock
    status: pending
  - id: report-module
    content: Add report.py with Rich Console matching old harness colors
    status: pending
  - id: wire-cli
    content: Cli.verbose + infer Setup/Test/Expect/Teardown; pass from conftest; run.sh -v -s
    status: pending
  - id: docs
    content: Update run.sh usage and README for transcript -v
    status: pending
isProject: false
---

# Rich verbose transcripts

`./black-box-tests/run.sh -v` today only enables pytest `-v` (test names). Wire it to also print **Setup / Test / Expect / Teardown** transcripts with Rich, matching the old [`harness/lib.sh`](black-box-tests/harness/lib.sh) color roles. Do not edit the 18 tests.

## When it prints

- Enable when pytest verbosity `>= 1` (`-v`).
- [`run.sh`](black-box-tests/run.sh) `-v` passes **both** `-v` and `-s` (disable capture so transcripts show on **passing** tests). Without `-s`, pytest swallows `Console.print` until a failure.
- `NO_COLOR` still disables harness color (Rich `no_color=True`). Child CLIs keep `NO_COLOR=1` in [`spawn_env`](black-box-tests/conftest.py) so transcripts stay uncolored DQL output inside dim blocks.

## Color map (old SGR → Rich)

Same roles as the bash runner:

- Test/case heading: yellow `Rule` + bold yellow title (`test_200_select_hash_key[dql]`)
- Binary: bold magenta (`dql` / `dqlrs`)
- Step labels (`Setup:`, `Test:`, `Expect:`, `Teardown:`): bold blue; dim `·` prefix
- DQL body / stdout body: dim
- Subheads `stdout` / `stderr`: grey / red; omit empty stderr when exit is 0
- Exit non-zero: red note

## Infer steps without changing tests

In [`cli.py`](black-box-tests/cli.py) `oneshot` / `assert_json` / `assert_stdout`, when `verbose`:

- `oneshot(..., json=False)` → **Setup** (DQL + stdout)
- `assert_json` / `assert_stdout` → **Test** (DQL + stdout) then **Expect** (JSON pretty or expected substring)
- Fixture teardown `DROP TABLE IF EXISTS` (`check=False`) → **Teardown**
- `--skip-teardown`: **Teardown** note `skipped (--skip-teardown)` (same as old harness)

Print a heading once per `Cli` instance (first spawn) so pytest’s own `PASSED` line stays; we do not reimplement PASS/FAIL banners.

## Files

- Add `rich` to [`black-box-tests/pyproject.toml`](black-box-tests/pyproject.toml) and refresh `uv.lock`.
- New [`black-box-tests/report.py`](black-box-tests/report.py): `Console` + `print_heading` / `print_step(name, dql, stdout, exitstatus, notes)` / `print_expect(text)`.
- [`Cli`](black-box-tests/cli.py): `verbose: bool`; after each spawn (and after expect in assert_*), call `report` if verbose. Pass `verbose` from the [`cli` fixture](black-box-tests/conftest.py) via `request.config.option.verbose >= 1`.
- [`run.sh`](black-box-tests/run.sh) + [`README.md`](black-box-tests/README.md): `-v` means pytest `-v -s` **and** CLI transcripts (not “later”).

Still no `import dql`. Tests stay as they are.
