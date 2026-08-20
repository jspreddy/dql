Developing
==========
This repository is a fork of `stevearc/dql <https://github.com/stevearc/dql>`__.
This guide covers the **Python** implementation in ``py-impl/``. To get started,
clone the fork::

    git clone https://github.com/jspreddy/dql.git

Some tests require `DynamoDB Local
<http://docs.aws.amazon.com/amazondynamodb/latest/developerguide/Tools.html>`_.
The test suite uses a pytest plugin that downloads and runs DynamoDB Local during
tests. It requires a Java runtime.

Local development with uv
-------------------------
This project uses `uv <https://docs.astral.sh/uv/>`_ for dependency management and
`taskipy <https://github.com/taskipy/taskipy>`_ for project tasks.

Prerequisites:

- `uv <https://docs.astral.sh/uv/getting-started/installation/>`_
- Java (for DynamoDB Local during tests)

Setup::

    # Install uv
    curl -LsSf https://astral.sh/uv/install.sh | sh

    # Clone and setup
    git clone https://github.com/jspreddy/dql.git
    cd dql/py-impl

    # Install dependencies
    uv sync --dev
    source .venv/bin/activate

From a git URL (subdirectory layout)::

    uv tool install --python 3.9 "git+https://github.com/jspreddy/dql.git#subdirectory=py-impl"

Project tasks
-------------
Run all ``uv`` / ``task`` commands from ``py-impl/``.

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
    uv run task dynamo            # install DynamoDB Local

Tests require DynamoDB Local. Start it before running tests::

    ./scripts/install_dynamodb_local.sh background

Multi-Python testing
--------------------
Supported Python versions: 3.9, 3.10, and 3.11. CI runs the lint and test jobs
against all three.

Run tests locally across all supported versions (requires DynamoDB Local)::

    uv run task test-matrix

To test a single version::

    uv python install 3.10
    UV_PYTHON=3.10 uv sync --dev
    UV_PYTHON=3.10 uv run task test

Versioning
----------
Use `bump2version` instead of `bumpversion` because `bump2version` is actively maintained. Configuration lives in ``py-impl/.bumpversion.cfg``.

Config based on: `<https://medium.com/@williamhayes/versioning-using-bumpversion-4d13c914e9b8>`_

Run bump2version from ``py-impl/`` (``scripts/bump-version.sh`` runs ``uv lock`` and amends the bump commit to include ``uv.lock``)::

    uv run task bump --dry-run build   # preview dev build bump
    uv run task bump --dry-run patch   # preview patch bump
    uv run task bump patch             # bump patch and reset to x.x.x-dev0
    uv run task bump minor             # bump minor and reset to x.x.x-dev0
    uv run task bump major             # bump major and reset to x.x.x-dev0
    uv run task bump build             # increment dev build (x.x.x-dev0 -> x.x.x-dev1)
    uv run task bump --tag release     # release as x.x.x (creates git tag)

Each bump updates ``py-impl/pyproject.toml``, ``py-docs/conf.py``, ``py-impl/dql/cli.py``, and ``py-impl/uv.lock``, and creates a git commit (``tag = False`` in config unless releasing with ``--tag release``).

``uv.lock`` stores PEP 440-normalized versions (for example ``0.6.4.dev10``), so it is not listed in ``.bumpversion.cfg``; ``uv lock`` regenerates it from ``pyproject.toml``.

Also update ``CHANGES.rst`` with release notes for the new version before committing or tagging a release.
