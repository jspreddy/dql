DQL
===
:Build: |build|_ |coverage|_
:Documentation: http://dql.readthedocs.org/
:Downloads: http://pypi.python.org/pypi/dql
:Source: https://github.com/stevearc/dql

.. |build| image:: https://github.com/stevearc/dql/actions/workflows/code-workflows.yml/badge.svg
.. _build: https://github.com/stevearc/dql/actions/workflows/code-workflows.yml
.. |coverage| image:: https://coveralls.io/repos/stevearc/dql/badge.png?branch=master
.. _coverage: https://coveralls.io/r/stevearc/dql?branch=master

A simple, SQL-ish language for DynamoDB

As of November 2020, Amazon has released `PartiQL
support <https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/ql-reference.html>`__
for DynamoDB. You should investigate that first to see if it addresses your
needs.

Getting Started
---------------
Installation can be done in a variety of ways

* An executable `pex <https://github.com/pantsbuild/pex>`__ file is available on `the release page <https://github.com/stevearc/dql/releases>`__.
* You can run a script to generate the pex file yourself: ``curl -o- install.py https://raw.githubusercontent.com/stevearc/dql/master/bin/install.py | python``
* With pip: ``pip install dql`` (To get the official version)
* If you want my development version, see branch `v-next`_

Prerequisites for development version: ``uv``

Installing from remote source code::

    # 1. Install uv
    curl -LsSf https://astral.sh/uv/install.sh | sh
    # 2. Install dql
    uv pip install git+https://github.com/jspreddy/dql.git@v-next

Install from local source code::

    # 1. Clone repo,
    git clone https://github.com/jspreddy/dql.git
    # 2. checkout branch `v-next`
    git checkout v-next
    # 3. Install dependencies and create virtual environment
    uv sync --dev
    uv venv
    # 4. Activate virtual environment
    source .venv/bin/activate
    # 5. Install in editable mode
    uv pip install -e .


Examples
--------

Here are some basic DQL examples to get you going:

Start the REPL::

    $ dql
    us-west-1>

Creating a table::

    us-west-1> CREATE TABLE forum_threads (name STRING HASH KEY,
             >                             subject STRING RANGE KEY,
             >                             THROUGHPUT (4, 2));

Inserting data::

    us-west-1> INSERT INTO forum_threads (name, subject, views, replies)
             > VALUES ('Self Defense', 'Defense from Banana', 67, 4),
             > ('Self Defense', 'Defense from Strawberry', 10, 0),
             > ('Cheese Shop', 'Anyone seen the camembert?', 16, 1);

Queries::

    us-west-1> SCAN * FROM forum_threads;
    us-west-1> SELECT count(*) FROM forum_threads WHERE name = 'Self Defense';
    us-west-1> SELECT * FROM forum_threads WHERE name = 'Self Defense';

Mutations::

    us-west-1> UPDATE forum_threads ADD views 1 WHERE
             > name = 'Self Defense' AND subject = 'Defense from Banana';
    us-west-1> DELETE FROM forum_threads WHERE name = 'Cheese Shop';

Changing tables::

    us-west-1> ALTER TABLE forum_threads SET THROUGHPUT (8, 4);
    us-west-1> DROP TABLE forum_threads;

And don't forget to use ``help``!


Developer/Maintainer Guide:
---------------------------
`See here for developer guide for v-next branch. <https://github.com/jspreddy/dql/blob/v-next/doc/topics/develop.rst>`

Migration to uv:
----------------
This project has been migrated from Poetry + pyenv + virtualenv to `uv <https://docs.astral.sh/uv/>`_, a fast Python package manager and installer.

For detailed migration information, see `UV_MIGRATION.md <UV_MIGRATION.md>`_.

Quick start for developers:

    # Install uv
    curl -LsSf https://astral.sh/uv/install.sh | sh

    # Clone and setup
    git clone https://github.com/jspreddy/dql.git
    cd dql
    git checkout v-next

    # Install dependencies
    uv sync --dev
    uv venv
    source .venv/bin/activate

    # Run tests
    uv run task test

    # Run linting
    uv run task lint

    # Format code
    uv run task format

## Complete Migration Plan: Poetry + pyenv + virtualenv → uv

I've created a comprehensive migration plan for your DQL project. Here's what has been implemented:

### 📁 Files Created/Modified

1. **`pyproject.toml`** - Updated with uv configuration
2. **`tox.ini`** - Modified to use uv instead of virtualenv-pyenv
3. **`.envrc`** - Updated to work with uv virtual environments
4. **`.github/workflows/code-workflows.yml`** - Updated CI/CD to use uv
5. **`migrate-to-uv.sh`** - Automated migration script
6. **`cleanup-old-tools.sh`** - Cleanup script for old configurations
7. **`UV_MIGRATION.md`** - Comprehensive migration guide
8. **`README.rst`** - Updated with uv installation instructions

### 🚀 Migration Steps

#### Phase 1: Preparation
1. **Install uv**: `curl -LsSf https://astral.sh/uv/install.sh | sh`
2. **Run migration script**: `./migrate-to-uv.sh`

#### Phase 2: Verification
1. **Test installation**: `uv run pytest tests`
2. **Test linting**: `uv run mypy dql tests bin/install.py`
3. **Verify CI/CD**: Push changes and check GitHub Actions

#### Phase 3: Cleanup (Optional)
1. **Remove old files**: `./cleanup-old-tools.sh`
2. **Update IDE settings** to use `.venv/bin/python`

### 🔄 Key Changes

| Component | Before | After |
|-----------|--------|-------|
| **Package Manager** | Poetry | uv |
| **Python Management** | pyenv | uv python |
| **Virtual Environment** | virtualenv | uv venv |
| **Lock File** | poetry.lock | uv.lock |
| **CI/CD** | tox + actions/setup-python | uv + astral-sh/setup-uv |

### 📋 New Commands

```bash
# Install dependencies
uv sync --dev

# Run commands
uv run pytest tests
uv run mypy dql tests bin/install.py

# Add dependencies
uv add requests
uv add --dev pytest-mock

# Create virtual environment
uv venv
```

### 🎯 Benefits

- **10-100x faster** dependency installation
- **Single tool** replaces multiple tools
- **Better dependency resolution**
- **Modern Rust-based architecture**
- **Simplified workflow**

### 📚 Documentation

- **`UV_MIGRATION.md`** - Complete migration guide
- **Updated README** - Quick start instructions
- **Migration scripts** - Automated setup

### ⚠️ Important Notes

1. **Backup files** are created during migration
2. **CI/CD pipeline** has been updated
3. **IDE configuration** may need updating
4. **Team members** should be notified of the change

The migration is designed to be **reversible** - you can always rollback to the previous setup if needed. All the necessary files and scripts are in place for a smooth transition to uv.

Would you like me to help you execute any specific part of this migration plan?
