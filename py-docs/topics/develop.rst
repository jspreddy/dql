Developing
==========
This repository is a fork of `stevearc/dql <https://github.com/stevearc/dql>`__.
This guide covers the **Python** implementation in ``py-impl/`` (the ``dql``
CLI). The repo also contains a Rust rewrite (``dqlrs``) in ``rust-impl/``;
Python ``dql`` is the recommended, stable client.

Clone the fork::

    git clone https://github.com/jspreddy/dql.git
    cd dql
    git checkout v-next

Local development with uv
-------------------------
This project uses `uv <https://docs.astral.sh/uv/>`_ for dependency management and
`taskipy <https://github.com/taskipy/taskipy>`_ for project tasks.

Prerequisites:

- Python 3.9, 3.10, or 3.11
- `uv <https://docs.astral.sh/uv/getting-started/installation/>`_
- Java (for DynamoDB Local during tests)

Setup::

    # Install uv
    curl -LsSf https://astral.sh/uv/install.sh | sh

    # Clone and setup
    git clone https://github.com/jspreddy/dql.git
    cd dql
    git checkout v-next
    cd py-impl

    # Install dependencies
    uv sync --dev
    source .venv/bin/activate

Editable install of the ``dql`` CLI onto your ``PATH``::

    uv tool install --python 3.9 --editable .

From a git URL (the package lives in the ``py-impl`` subdirectory)::

    uv tool install --python 3.9 \
      "git+https://github.com/jspreddy/dql.git@v-next#subdirectory=py-impl"

Project tasks
-------------
Run all ``uv`` / ``task`` commands from ``py-impl/``. Shared helper scripts live
in ``scripts/`` at the repository root.

List available tasks::

    uv run task --list

Common commands::

    uv run task test              # run all tests
    uv run task test-matrix       # run tests on Python 3.9, 3.10, and 3.11
    uv run task test-verbose      # run tests with verbose output
    uv run task test-specific tests/test_parser.py  # run a specific test file
    uv run task lint              # run mypy, isort, black, pylint
    uv run task fix               # format code with isort and black
    uv run task coverage          # run tests with HTML coverage report (htmlcov/)
    uv run task package           # build package and pex binary
    uv run task dynamo            # download (if needed) and start DynamoDB Local

DynamoDB Local
--------------
Tests connect to DynamoDB Local on ``localhost:8000``. They do not start the
server themselves. Java is required.

The installer is ``scripts/install_dynamodb_local.sh``. It downloads the jar
into ``.dynamo-local/`` at the **repository root** (shared with the Rust
implementation) and starts Java. From ``py-impl/``::

    ../scripts/install_dynamodb_local.sh              # foreground
    ../scripts/install_dynamodb_local.sh background   # background
    uv run task dynamo                                # same as foreground

Leave Local running while you test (a backgrounded ``&`` child is killed when
that shell exits). Confirm::

    nc -z localhost 8000 && echo "DynamoDB Local is up"

Multi-Python testing
--------------------
Supported Python versions: 3.9, 3.10, and 3.11
(``requires-python = ">=3.9,<3.12"``). CI runs lint and tests on all three.

Run tests locally across all supported versions (requires DynamoDB Local)::

    uv run task test-matrix

To test a single version::

    uv python install 3.10
    UV_PYTHON=3.10 uv sync --dev
    UV_PYTHON=3.10 uv run task test

Building docs
-------------
Sphinx sources are in ``py-docs/``. From ``py-impl/`` after ``uv sync --dev``::

    cd ../py-docs
    make html

``make html`` copies ``py-impl/CHANGES.rst`` into ``py-docs/changes.rst`` and
writes HTML to ``py-docs/_build/html/``.

Versioning
----------
Python and Rust versions bump independently. Python tags are
``python-{version}`` (for example ``python-0.6.4``) so they do not collide with
Rust ``rust-{version}`` tags.

Use `bump2version` instead of `bumpversion` because `bump2version` is actively
maintained. Configuration lives in ``py-impl/.bumpversion.cfg``.

Config based on: `<https://medium.com/@williamhayes/versioning-using-bumpversion-4d13c914e9b8>`_

From ``py-impl/``, ``uv run task bump`` runs ``../scripts/bump-version.sh python``,
which runs ``uv lock`` and amends the bump commit to include ``uv.lock``. You
can also call the script from the repository root::

    ./scripts/bump-version.sh python --dry-run build

From ``py-impl/``::

    uv run task bump --dry-run build   # preview dev build bump
    uv run task bump --dry-run patch   # preview patch bump
    uv run task bump patch             # bump patch and reset to x.x.x-dev0
    uv run task bump minor             # bump minor and reset to x.x.x-dev0
    uv run task bump major             # bump major and reset to x.x.x-dev0
    uv run task bump build             # increment dev build (x.x.x-dev0 -> x.x.x-dev1)
    uv run task bump --tag release     # release as x.x.x (creates git tag)

Each bump updates ``py-impl/pyproject.toml``, ``py-docs/conf.py``,
``py-impl/dql/cli.py``, and ``py-impl/uv.lock``, and creates a git commit
(``tag = False`` in config unless releasing with ``--tag release``).

``uv.lock`` stores PEP 440-normalized versions (for example ``0.6.4.dev10``), so
it is not listed in ``.bumpversion.cfg``; ``uv lock`` regenerates it from
``pyproject.toml``.

Update ``py-impl/CHANGES.rst`` with release notes for the new version before
committing or tagging a release.
