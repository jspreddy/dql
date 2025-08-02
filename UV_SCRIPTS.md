# UV Scripts Migration Guide

This document maps all the tox commands to their uv equivalents.

## Tox to UV Command Mapping

```bash
uv run task --list
```

| Tox Command | UV Command | Description |
|-------------|------------|-------------|
| `tox -e test` | `uv run task test` | Run all tests |
| `tox -e test -- --verbose` | `uv run task test-verbose` | Run tests with verbose output |
| `tox -e test -- tests/test_specific.py` | `uv run task test-specific tests/test_specific.py` | Run specific test file |
| `tox -e lint` | `uv run task lint` | Run all linting tools |
| `tox -e format` | `uv run task format` | Format code with isort and black |
| `tox -e coverage` | `uv run task coverage` | Run tests with coverage |
| `tox -e coverage-ci` | `uv run task coverage-ci` | Run coverage as in CI |
| `tox -e package` | `uv run task package` | Build package with pex |


## Benefits of UV taskipy tasks

1. **Faster**: No virtual environment creation overhead
2. **Simpler**: Direct command execution
3. **Consistent**: Same environment for all commands
4. **Flexible**: Easy to add new scripts
5. **Portable**: Works across different platforms

## Adding New Scripts

To add a new script, edit `pyproject.toml` and add to the `[tool.taskipy.tasks]` section:

```toml
[tool.taskipy.tasks]
new-task = "command to run"
```
(more info) https://github.com/taskipy/taskipy?tab=readme-ov-file

Then run with:
```bash
uv run task new-task
```
