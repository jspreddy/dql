Developing
==========
To get started developing dql, clone the repo::

    git clone https://github.com/stevearc/dql.git

It is recommended that you create a virtualenv to develop::

    # python 3
    python3 -m venv dql_env
    # python 2
    virtualenv dql_env

    source ./dql_env/bin/activate
    pip install -e .

Running Tests
-------------
The command to run tests is ``python setup.py nosetests``, but I recommend using
`tox <https://tox.readthedocs.io/en/latest/>`__. Some of these tests require
`DynamoDB Local
<http://docs.aws.amazon.com/amazondynamodb/latest/developerguide/Tools.html>`_.
There is a nose plugin that will download and run the DynamoDB Local service
during the tests. It requires the java 6/7 runtime, so make sure you have that
installed.


Local dev Using ``pyenv pyenv-virtualenv tox tox-pyenv``
--------------------------------------------------------

Pre-requisites

- Install `pyenv <https://github.com/pyenv/pyenv>`_
    - Why use pyenv? `Intro to pyenv <https://realpython.com/intro-to-pyenv/#what-about-a-package-manager>`_
- Install `pyenv-virtualenv <https://github.com/pyenv/pyenv-virtualenv#installing-with-homebrew-for-macos-users>`_ so that you can manage virtualenvs from pyenv.
- Install Java: I recomend using `sdkman <https://sdkman.io/install>`_ to manage your java installations.
    - I use java version 8.0.265.j9-adpt
    - ``sdk install java 8.0.265.j9-adpt``

Setting up local envs::

    # See installed python versions
    pyenv versions

    # See which python you are currently using. This will also show missing versions
    # required by .python-version file.
    pyenv which python

    # Install the required python versions using pyenv.
    pyenv install <version-number>

        # Version numbers are listed in the .python-version file.
        pyenv install 3.9.21

    # Create a virtual env named "dql-local-env" with python version 3.9.21
    pyenv virtualenv 3.9.21 dql-local-env

    # Look at the virtual envs. dql-local-env should have a * next to it indicating
    # that it is selected.
    pyenv virtualenvs

    # You should be currently using "~/.pyenv/versions/dql-local-env/bin/python"
    pyenv which python

    # install dependencies
    pip install -r requirements_dev.txt

    # running tests with tox
    tox

        # running specific tox env
        tox -e format
        tox -e lint
        tox -e package

After setting up your local env, you can install the executable of dql::

    pip install -e .

    # In case you have a global dql already installed for your day to day use,
    # I recommend bumping the patch number so that you know which version you
    # are currently executing.
    bump2version patch

    # check the version
    dql --version

    # To install with pyenv & pipx, package with `tox -e package` first, then:
    pyenv local 3.9.21
    pyenv which python
    pipx install --python $(pyenv which python) ./dist/filename.tar.gz


Migration to uv
---------------
This project has been migrated from Poetry + pyenv + virtualenv to `uv <https://docs.astral.sh/uv/>`_, a fast Python package manager and installer.

For detailed migration information, see ``UV_MIGRATION.md`` at the repository root.

Quick start for developers::

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

Each bump updates ``pyproject.toml``, ``doc/conf.py``, ``dql/cli.py``, and ``uv.lock``, and creates a git commit (``tag = False`` in config unless releasing with ``--tag release``).

``uv.lock`` stores PEP 440-normalized versions (for example ``0.6.4.dev10``), so it is not listed in ``.bumpversion.cfg``; ``uv lock`` regenerates it from ``pyproject.toml``.

Also update ``CHANGES.rst`` with release notes for the new version before committing or tagging a release.

