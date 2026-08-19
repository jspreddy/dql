DQL
===
:Build: |build|_
:Documentation: http://dql.readthedocs.org/
:Downloads: http://pypi.python.org/pypi/dql
:Source: https://github.com/jspreddy/dql
:Upstream: https://github.com/stevearc/dql

.. |build| image:: https://github.com/jspreddy/dql/actions/workflows/code-workflows.yml/badge.svg
.. _build: https://github.com/jspreddy/dql/actions/workflows/code-workflows.yml

A simple, SQL-ish language for DynamoDB

This repository (`jspreddy/dql <https://github.com/jspreddy/dql>`__) is a
**fork** of `stevearc/dql <https://github.com/stevearc/dql>`__. Use this repo
for clone, install-from-source, and GitHub release URLs. The PyPI package and
Read the Docs site still belong to upstream.

As of November 2020, Amazon has released `PartiQL
support <https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/ql-reference.html>`__
for DynamoDB. You should investigate that first to see if it addresses your
needs.

Getting Started
---------------
Installation can be done in a variety of ways.

Rust binary (recommended on the ``v-rust`` branch)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Download a prebuilt binary from `GitHub releases <https://github.com/jspreddy/dql/releases>`__::

    curl -fsSL https://raw.githubusercontent.com/jspreddy/dql/v-rust/bin/install-rust.sh | sh

Or build and install from a release tag with Cargo::

    cargo install --git https://github.com/jspreddy/dql.git --tag 0.6.4 --locked -p dql-cli --root ~/.local

The binary is installed as ``~/.local/bin/dqlrs``. Add that directory to your ``PATH`` if needed.

For local development of the Rust implementation, see ``rust-impl/README.md``.

Legacy Python installation
~~~~~~~~~~~~~~~~~~~~~~~~~~

The Python implementation remains available while the Rust rewrite completes.

* An executable `pex <https://github.com/pantsbuild/pex>`__ file is available on `this fork's release page <https://github.com/jspreddy/dql/releases>`__.
* You can run a script to generate the pex file yourself: ``curl -fsSL https://raw.githubusercontent.com/jspreddy/dql/master/bin/install.py | sh``
* With pip: ``pip install dql`` (upstream package on PyPI)
* If you want the Python development version, see branch `v-next`_

Prerequisites for Python development version: ``uv``

Installing from remote source code::

    # 1. Install uv
    curl -LsSf https://astral.sh/uv/install.sh | sh
    # 2. Install dql
    uv tool install --python 3.9.21 git+https://github.com/jspreddy/dql.git@v-next

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

