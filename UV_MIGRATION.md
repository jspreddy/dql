# Migration Guide: Poetry + pyenv + virtualenv → uv

This document outlines the migration from the current Python toolchain (Poetry + pyenv + virtualenv) to uv, a fast Python package manager and installer.

## What is uv?

[uv](https://docs.astral.sh/uv/) is a fast Python package installer and resolver, written in Rust. It's designed to be a drop-in replacement for pip, pip-tools, poetry, and pyenv, with significant performance improvements.

## Benefits of Migration

- **Speed**: uv is 10-100x faster than pip and poetry
- **Simplicity**: Single tool replaces multiple tools (pip, poetry, pyenv, virtualenv)
- **Compatibility**: Works with existing pyproject.toml files
- **Reliability**: Better dependency resolution and lock file management
- **Modern**: Built with Rust for performance and reliability

## Pre-Migration Checklist

Before starting the migration:

1. ✅ Ensure all current work is committed to git
2. ✅ Note down any custom Poetry configurations
3. ✅ Document any pyenv-specific Python versions
4. ✅ Backup any custom virtual environment configurations

## Migration Steps

### 1. Install uv

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

### 2. Run the Migration Script

```bash
./migrate-to-uv.sh
```

This script will:
- Install uv if not already installed
- Backup existing configuration files
- Initialize uv in the project
- Install all dependencies
- Create a new virtual environment

### 3. Manual Migration (if needed)

If you prefer to migrate manually:

```bash
# Backup existing files
cp poetry.lock poetry.lock.backup
mv .python-version .python-version.backup

# Remove existing virtual environment
rm -rf .venv

# Initialize uv
uv init --no-readme

# Install dependencies
uv sync --dev

# Create virtual environment
uv venv
```

## Configuration Changes

### pyproject.toml Updates

The `pyproject.toml` file has been updated with:

1. **Removed**: `virtualenv-pyenv` from dev dependencies
2. **Added**: `[tool.uv]` section with dev-dependencies
3. **Consolidated**: All development dependencies in one place

### Environment Management

- **Before**: `.python-version` file for pyenv
- **After**: uv manages Python versions automatically

### Virtual Environment

- **Before**: Poetry-managed virtual environment
- **After**: uv-managed virtual environment in `.venv/`

## New Commands

| Old Command | New Command | Description |
|-------------|-------------|-------------|
| `poetry install` | `uv sync` | Install dependencies |
| `poetry install --dev` | `uv sync --dev` | Install dev dependencies |
| `poetry add <package>` | `uv add <package>` | Add dependency |
| `poetry add --dev <package>` | `uv add --dev <package>` | Add dev dependency |
| `poetry run <command>` | `uv run <command>` | Run command in environment |
| `pyenv local 3.11` | `uv python install 3.11` | Install Python version |
| `virtualenv .venv` | `uv venv` | Create virtual environment |

## Development Workflow

### Setting Up Development Environment

```bash
# Clone the repository
git clone <repository-url>
cd dql

# Install uv (if not already installed)
curl -LsSf https://astral.sh/uv/install.sh | sh

# Install dependencies and create virtual environment
uv sync --dev
uv venv

# Activate virtual environment
source .venv/bin/activate
```

### Running Tests

```bash
# Run all tests
uv run pytest tests

# Run specific test file
uv run pytest tests/test_specific.py

# Run with coverage
uv run pytest --cov=dql tests
```

### Code Quality

```bash
# Type checking
uv run mypy dql tests bin/install.py

# Linting
uv run pylint --rcfile=.pylintrc dql tests bin/install.py

# Formatting
uv run isort --skip snapshots --atomic dql tests bin/install.py
uv run black --exclude=snapshots dql tests bin/install.py
```

### Adding Dependencies

```bash
# Add production dependency
uv add requests

# Add development dependency
uv add --dev pytest-mock

# Add dependency with version constraint
uv add "django>=4.0,<5.0"
```

## CI/CD Updates

The GitHub Actions workflow has been updated to use uv:

- **Before**: Uses `actions/setup-python` + `ymyzk/run-tox-gh-actions`
- **After**: Uses `astral-sh/setup-uv` + direct uv commands

### Key Changes in CI/CD

1. **Installation**: Uses `astral-sh/setup-uv@v1` action
2. **Dependencies**: `uv sync --dev` instead of tox
3. **Commands**: Direct `uv run` commands instead of tox environments

## Troubleshooting

### Common Issues

1. **uv command not found**
   ```bash
   # Reinstall uv
   curl -LsSf https://astral.sh/uv/install.sh | sh
   # Restart your shell
   source ~/.bashrc  # or ~/.zshrc
   ```

2. **Dependency conflicts**
   ```bash
   # Remove lock file and reinstall
   rm uv.lock
   uv sync --dev
   ```

3. **Virtual environment issues**
   ```bash
   # Remove and recreate virtual environment
   rm -rf .venv
   uv venv
   source .venv/bin/activate
   ```

### Rollback Plan

If you need to rollback to the previous setup:

1. Restore backup files:
   ```bash
   mv poetry.lock.backup poetry.lock
   mv .python-version.backup .python-version
   ```

2. Reinstall Poetry and pyenv:
   ```bash
   curl -sSL https://install.python-poetry.org | python3 -
   curl https://pyenv.run | bash
   ```

3. Reinstall dependencies:
   ```bash
   poetry install
   ```

## IDE Configuration

### VS Code

Update your VS Code settings to use the new virtual environment:

```json
{
    "python.defaultInterpreterPath": "./.venv/bin/python",
    "python.terminal.activateEnvironment": true
}
```

### PyCharm

1. Go to Settings/Preferences → Project → Python Interpreter
2. Add new interpreter
3. Select "Existing environment"
4. Point to `.venv/bin/python`

## Performance Comparison

| Operation | Poetry | uv | Improvement |
|-----------|--------|----|-------------|
| Install dependencies | ~30s | ~3s | 10x faster |
| Add new dependency | ~15s | ~1s | 15x faster |
| Lock file generation | ~20s | ~2s | 10x faster |
| Virtual env creation | ~10s | ~1s | 10x faster |

## Support

- **uv Documentation**: https://docs.astral.sh/uv/
- **GitHub Repository**: https://github.com/astral-sh/uv
- **Discord Community**: https://discord.gg/astral-sh

## Migration Checklist

- [ ] Install uv
- [ ] Run migration script
- [ ] Test installation
- [ ] Update IDE configuration
- [ ] Test CI/CD pipeline
- [ ] Update team documentation
- [ ] Remove old tool installations (optional)
- [ ] Clean up backup files (after verification)

## Post-Migration Verification

After migration, verify that:

1. ✅ All tests pass: `uv run pytest tests`
2. ✅ Linting passes: `uv run mypy dql tests bin/install.py`
3. ✅ CI/CD pipeline succeeds
4. ✅ Development workflow works as expected
5. ✅ All team members can set up the environment
