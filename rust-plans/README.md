# Rust Rewrite Plans

This folder contains high-level plans for rewriting DQL in Rust while preserving
the current user-facing behavior of the Python implementation.

## Current product shape

DQL is a SQL-like CLI and library for DynamoDB. The current implementation has
these major surfaces:

- CLI entrypoint and REPL: `dql/__init__.py` and `dql/cli.py`
- Query execution and DynamoDB integration: `dql/engine.py`
- Grammar and parse tree construction: `dql/grammar/`
- Expression AST and DynamoDB expression rendering: `dql/expressions/`
- Table and index metadata: `dql/models.py`
- Output, paging, JSON handling, history, help, and monitoring: `dql/output.py`,
  `dql/history.py`, `dql/help.py`, and `dql/monitor.py`
- Compatibility tests: `tests/test_parser.py`, `tests/test_queries.py`,
  `tests/test_engine.py`, `tests/test_cli.py`, and related focused tests

## Rewrite goals

1. Preserve DQL syntax and command behavior before expanding the language.
2. Keep parser, planner, executor, and terminal UI concerns isolated.
3. Use typed Rust domain models instead of carrying Python parse result shapes
   across layers.
4. Treat the existing Python tests and documentation examples as the behavioral
   oracle for the first Rust implementation.
5. Make DynamoDB Local integration tests the main proof that the Rust engine is
   compatible with existing query behavior.

## Plan documents

- `architecture.md` describes the proposed Rust crate and module boundaries.
- `migration-roadmap.md` breaks the rewrite into compatibility-focused phases.
- `migration-from-python.md` documents install and behavior differences for users moving from Python to Rust.
- `testing-strategy.md` defines the parity gates and test migration approach.

## Non-goals for the first rewrite pass

- Changing DQL syntax.
- Adding new DynamoDB features before parity is established.
- Replacing the documented CLI workflow with an incompatible interface.
- Preserving Python-specific serialization formats such as pickle unless a
  compatibility requirement is explicitly accepted.
