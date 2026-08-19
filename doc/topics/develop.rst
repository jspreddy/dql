Developing
==========
This repository is a fork of `stevearc/dql <https://github.com/stevearc/dql>`__.
To get started developing this fork, clone it::

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
    cd dql
    git checkout v-next

    # Install dependencies
    uv sync --dev
    source .venv/bin/activate

Project tasks
-------------
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
Use `bump2version` instead of `bumpversion` because `bump2version` is actively maintained. Configuration lives in ``.bumpversion.cfg``.

Config based on: `<https://medium.com/@williamhayes/versioning-using-bumpversion-4d13c914e9b8>`_

Run bump2version through the project environment (``scripts/bump-version.sh`` runs ``uv lock`` and amends the bump commit to include ``uv.lock``)::

    uv run task bump --dry-run build   # preview dev build bump
    uv run task bump --dry-run patch   # preview patch bump
    uv run task bump patch             # bump patch and reset to x.x.x-dev0
    uv run task bump minor             # bump minor and reset to x.x.x-dev0
    uv run task bump major             # bump major and reset to x.x.x-dev0
    uv run task bump build             # increment dev build (x.x.x-dev0 -> x.x.x-dev1)
    uv run task bump --tag release     # release as x.x.x (creates git tag)

Each bump updates ``pyproject.toml``, ``doc/conf.py``, ``dql/cli.py``, ``rust-impl/Cargo.toml``, ``uv.lock``, and ``rust-impl/Cargo.lock``, and creates a git commit (``tag = False`` in config unless releasing with ``--tag release``).

``uv.lock`` stores PEP 440-normalized versions (for example ``0.6.4.dev10``), so it is not listed in ``.bumpversion.cfg``; ``uv lock`` regenerates it from ``pyproject.toml``. ``rust-impl/Cargo.lock`` is regenerated with ``cargo generate-lockfile`` after each bump.

Also update ``CHANGES.rst`` with release notes for the new version before committing or tagging a release.

Rust implementation
-------------------
The Rust rewrite lives in ``rust-impl/`` on the ``v-rust`` branch.

Local checks::

    cd rust-impl
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    cargo build --release -p dql-cli
    ./scripts/smoke_test.sh

Release checklist for Rust binaries:

1. Bump the shared version with ``uv run task bump --tag release``.
2. Merge to ``v-rust``.
3. Push the release tag; GitHub Actions builds Linux and macOS archives.
4. Verify the uploaded release assets with ``rust-impl/scripts/smoke_test.sh``.

See ``rust-plans/migration-from-python.md`` for user-facing migration notes.
