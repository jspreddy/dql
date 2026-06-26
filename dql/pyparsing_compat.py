"""Compatibility shims for pyparsing 2.1.4 on Python 3.10+.

DQL pins ``pyparsing==2.1.4`` because the grammar and expression builders
(``dql/grammar/``, ``UpdateExpression.from_update``, etc.) were written against
that release's API and parse-result layout. Newer pyparsing 2.x releases rename
symbols (for example ``signedInteger`` → ``signed_integer``) and change how
named parse results are exposed on nested ``ParseResults``, which breaks update
expression handling without a larger grammar refactor.

Python 3.10 removed several ABC aliases from the top-level ``collections``
module (they live in ``collections.abc`` only). Pyparsing 2.1.4 still references
``collections.MutableMapping`` (at import time, for ``ParseResults``) and
``collections.Sequence`` (when combining parser expressions). Importing
pyparsing on 3.10+ without this shim raises ``AttributeError``.

Upgrading pyparsing was attempted; 2.4.x fixes the ``collections`` issue but
changes parse-result structure enough that update statements parse yet produce
empty expressions. Keeping 2.1.4 plus these small shims is the minimal fix for
3.9–3.11 support.

This module must be imported before any ``import pyparsing`` (see
``tests/conftest.py``, ``dql/grammar/__init__.py``, ``dql/cli.py``, and
``dql/engine.py``).
"""

import collections
import collections.abc

for _name in ("MutableMapping", "Sequence"):
    if not hasattr(collections, _name):
        setattr(collections, _name, getattr(collections.abc, _name))
