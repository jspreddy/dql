---
name: Rich verbose transcripts
overview: When -v is set, print a Rich transcript per test — heading, docstring description, binary subheading, then each step as a list item with syntax-highlighted input/output/expect blocks. Commit after each work todo (separate commit todos).
todos:
  - id: dep-rich
    content: Add rich to black-box-tests/pyproject.toml and uv.lock
    status: completed
  - id: commit-dep-rich
    content: "Commit: test(black-box-tests): add rich for verbose transcripts"
    status: completed
  - id: report-module
    content: Add report.py — heading, description, binary subheading, list-item steps with Syntax blocks
    status: completed
  - id: commit-report-module
    content: "Commit: test(black-box-tests): add Rich transcript reporter"
    status: completed
  - id: docstrings
    content: Add a one-line docstring to each of the 18 tests for the verbose description
    status: completed
  - id: commit-docstrings
    content: "Commit: test(black-box-tests): add verbose descriptions to acceptance tests"
    status: completed
  - id: wire-cli
    content: Cli.verbose prints Setup/Test/Output/Expect/Teardown; conftest prints heading; run.sh -v -s
    status: completed
  - id: commit-wire-cli
    content: "Commit: test(black-box-tests): print Rich transcripts when pytest is verbose"
    status: completed
  - id: docs
    content: Update run.sh usage and README for transcript -v
    status: completed
  - id: commit-docs
    content: "Commit: docs(black-box-tests): document -v CLI transcripts"
    status: completed
isProject: false
---

# Rich verbose transcripts

`./black-box-tests/run.sh -v` today only enables pytest `-v` (test names). Print a **structured Rich transcript** for each test, including a plain-language description and syntax-highlighted blocks.

## Layout (what `-v` prints)

```text
# test_200_select_hash_key          ← heading (test function name, no [dql])

Query a table by hash key.          ← docstring (plain text)

## dql                              ← binary subheading

- Setup                             ← list item
      CREATE TABLE ...              ← Syntax("sql")
- Output
      (CLI stdout; json lexer if --json)
- Test
      SELECT * FROM ...             ← Syntax("sql")
- Output
      [{"id": "a", "n": 1}]         ← Syntax("json")
- Expect
      [{"id": "a", "n": 1}]         ← Syntax("json")
- Teardown
      DROP TABLE IF EXISTS ...      ← Syntax("sql")
- Output
      Dropped table                 ← Syntax("text")
```

Then pytest’s usual `PASSED` / `FAILED` line.

Rich mapping:

- Heading: `console.rule` + bold yellow `# test_…` (or `Markdown` `#`)
- Description: unstyled / dim paragraph under the heading (`inspect.cleandoc` of the test docstring)
- Binary: magenta `## dql` / `## dqlrs`
- Steps: bullet (`• Setup`) in bold blue
- Bodies: `rich.syntax.Syntax(code, lexer, theme="ansi_dark", word_wrap=True)` indented under the bullet
  - DQL input (`Setup` / `Test` / `Teardown`) → `sql`
  - `--json` stdout and JSON expect → `json`
  - other stdout / text expect → `text`

`NO_COLOR` disables Rich color. Child CLIs still get `NO_COLOR=1` so highlighted blocks are the harness’s, not the CLI’s own ANSI.

## When it prints

- Pytest verbosity `>= 1` (`-v`).
- [`run.sh`](black-box-tests/run.sh) `-v` passes **`-v -s`**. Without `-s`, pytest captures `Console.print` on passing tests.

## Who prints what

[`conftest.py`](black-box-tests/conftest.py) `cli` fixture (start of each test):

- Heading = function name (`test_200_select_hash_key`)
- Description = that function’s docstring (required; see below)
- Binary subheading = `cli.label`

[`cli.py`](black-box-tests/cli.py) after each spawn / assert, when `verbose`:

- `oneshot(json=False)` → **Setup** (input sql) + **Output**
- `assert_json` / `assert_stdout` → **Test** (input sql) + **Output** + **Expect**
- Fixture `DROP TABLE IF EXISTS` → **Teardown** (input sql) + **Output**
- `--skip-teardown` → **Teardown** list item with note `skipped (--skip-teardown)` (no code block)

Empty stdout: show a dim `(empty)` instead of a blank Syntax block. Omit stderr unless exit is non-zero (then a **stderr** list item, `text` lexer).

## Docstrings

The 18 tests currently have no docstrings. Add one sentence each (from the old case READMEs / [ordering.md](black-box-tests/ordering.md)), e.g.:

```python
def test_200_select_hash_key(cli: Cli) -> None:
    """Query a table by hash key through the CLI against DynamoDB Local."""
```

If a docstring is missing at runtime, print `(no description)` in dim so `-v` never crashes.

## Files

- `rich` in [`pyproject.toml`](black-box-tests/pyproject.toml); `uv lock`
- New [`black-box-tests/report.py`](black-box-tests/report.py): `print_test_header(name, description, binary)`, `print_step(title, code, lexer)`
- Wire `Cli.verbose` from `request.config.option.verbose >= 1`
- README / `run.sh` help: `-v` is transcripts, not “later”

Still no `import dql`.

## Commits

Each work todo is followed by a **separate commit todo**. Do not start the next work item until that commit is done. Do not bundle work todos into one commit.

- After **dep-rich** → **commit-dep-rich**: `test(black-box-tests): add rich for verbose transcripts`
- After **report-module** → **commit-report-module**: `test(black-box-tests): add Rich transcript reporter`
- After **docstrings** → **commit-docstrings**: `test(black-box-tests): add verbose descriptions to acceptance tests`
- After **wire-cli** → **commit-wire-cli**: `test(black-box-tests): print Rich transcripts when pytest is verbose`
- After **docs** → **commit-docs**: `docs(black-box-tests): document -v CLI transcripts`
