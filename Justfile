# Dev tasks for the DQL repo. Run `just` from anywhere in the checkout.

# List dev tasks
default:
    @just --list

# Start DynamoDB Local in the background when port 8000 is down
[group('local')]
dynamo:
    cd "{{justfile_directory()}}" && ./scripts/install_dynamodb_local.sh background

# List running DynamoDB Local processes and stop the ones you choose
[group('local')]
dynamo-stop:
    "{{justfile_directory()}}/scripts/stop_dynamodb_local.sh"

# Sync the Python dev environment
[group('python')]
py-sync:
    cd "{{justfile_directory()}}/py-impl" && uv sync --dev

# Lint Python (mypy, isort, black, pylint)
[group('python')]
py-lint:
    cd "{{justfile_directory()}}/py-impl" && uv run task lint

# Format Python with isort and black
[group('python')]
py-fix:
    cd "{{justfile_directory()}}/py-impl" && uv run task fix

# Run Python tests (DynamoDB Local on port 8000)
[group('python')]
py-test:
    cd "{{justfile_directory()}}/py-impl" && uv run task test

# Build the Python sdist and wheel
[group('python')]
py-build:
    cd "{{justfile_directory()}}/py-impl" && uv build

# Install dql as an editable uv tool (~/.local/bin/dql)
[group('python')]
py-install:
    cd "{{justfile_directory()}}/py-impl" && uv tool install --python 3.9 --editable --force .

# Sync, lint, test, and build the Python package
[group('python')]
py: py-sync py-lint py-test py-build

# Fetch Rust crates from Cargo.lock
[group('rust')]
rust-fetch:
    cd "{{justfile_directory()}}/rust-impl" && cargo fetch --locked

# Check Rust formatting
[group('rust')]
rust-fmt:
    cd "{{justfile_directory()}}/rust-impl" && cargo fmt --check

# Lint Rust with clippy
[group('rust')]
rust-clippy:
    cd "{{justfile_directory()}}/rust-impl" && cargo clippy --workspace --all-targets -- -D warnings

# Run Rust workspace tests (DynamoDB Local on port 8000 for integration tests)
[group('rust')]
rust-test:
    cd "{{justfile_directory()}}/rust-impl" && cargo test --workspace

# Run Rust DynamoDB Local smoke and parity tests
[group('rust')]
rust-local:
    cd "{{justfile_directory()}}/rust-impl" && cargo test -p dql-engine --test dynamodb_local_smoke && cargo test -p dql-engine --test dynamodb_local_parity

# Build the release dqlrs binary
[group('rust')]
rust-build:
    cd "{{justfile_directory()}}/rust-impl" && cargo build --release -p dql-cli

# Install dqlrs to ~/.local/bin
[group('rust')]
rust-install:
    cd "{{justfile_directory()}}/rust-impl" && cargo install --path crates/dql-cli --locked --force --root "{{home_directory()}}/.local"

# Smoke-test the release dqlrs binary (fails if DynamoDB Local is down)
[group('rust')]
rust-smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    export DQL_BIN="{{justfile_directory()}}/rust-impl/target/release/dqlrs"
    export DQL_REQUIRE_LOCAL=1
    ./scripts/rust-smoke-test.sh

# Fetch, lint, test, build, and smoke-test Rust
[group('rust')]
rust: rust-fetch rust-fmt rust-clippy rust-test rust-local rust-build rust-smoke

# Run black-box acceptance tests. Extra args are passed to black-box-tests/run.sh.
[group('tests')]
black-box *args:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    root="{{justfile_directory()}}"
    if [[ -x "$root/py-impl/.venv/bin/dql" ]]; then
        export DQL_BIN="$root/py-impl/.venv/bin/dql"
    fi
    if [[ -x "$root/rust-impl/target/release/dqlrs" ]]; then
        export DQLRS_BIN="$root/rust-impl/target/release/dqlrs"
    fi
    # `just recipe -- --flag` includes the separator in the variadic args.
    args=({{args}})
    if [[ ${#args[@]} -gt 1 && "${args[0]}" == "--" ]]; then
        args=("${args[@]:1}")
    elif [[ ${#args[@]} -eq 1 && "${args[0]}" == "--" ]]; then
        args=()
    fi
    if [[ ${#args[@]} -gt 0 ]]; then
        ./black-box-tests/run.sh "${args[@]}"
    else
        ./black-box-tests/run.sh
    fi

# Dry-run the notebook env and kernel registration
[group('notebook')]
notebook-dry-run:
    cd "{{justfile_directory()}}" && ./notebook/start.sh --dry-run

# Start JupyterLab. Extra args are passed to notebook/start.sh.
[group('notebook')]
notebook *args:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    args=({{args}})
    if [[ ${#args[@]} -gt 1 && "${args[0]}" == "--" ]]; then
        args=("${args[@]:1}")
    elif [[ ${#args[@]} -eq 1 && "${args[0]}" == "--" ]]; then
        args=()
    fi
    if [[ ${#args[@]} -gt 0 ]]; then
        ./notebook/start.sh "${args[@]}"
    else
        ./notebook/start.sh
    fi

# Playwright notebook tests. Flags need a `--` separator: just notebook-test -- --headed -- tests/test_dql_notebook.py::test_launcher_lists_dql_kernels
[group('notebook')]
notebook-test *args:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    root="{{justfile_directory()}}"
    if [[ -x "$root/py-impl/.venv/bin/dql" ]]; then
        export DQL_BIN="$root/py-impl/.venv/bin/dql"
    fi
    if [[ -x "$root/rust-impl/target/release/dqlrs" ]]; then
        export DQLRS_BIN="$root/rust-impl/target/release/dqlrs"
    elif [[ -x "$root/rust-impl/target/debug/dqlrs" ]]; then
        export DQLRS_BIN="$root/rust-impl/target/debug/dqlrs"
    fi
    # `just recipe -- --flag` includes the separator in the variadic args.
    args=({{args}})
    if [[ ${#args[@]} -gt 1 && "${args[0]}" == "--" ]]; then
        args=("${args[@]:1}")
    elif [[ ${#args[@]} -eq 1 && "${args[0]}" == "--" ]]; then
        args=()
    fi
    if [[ ${#args[@]} -gt 0 ]]; then
        ./notebook-tests/run.sh "${args[@]}"
    else
        ./notebook-tests/run.sh
    fi

# Start DQLRS Web against dqlrs-web/examples. Extra args go to dqlrs-web/run.sh: just web -- --port 9000
[group('web')]
web *args:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    # `just recipe -- --flag` includes the separator in the variadic args.
    args=({{args}})
    if [[ ${#args[@]} -gt 0 && "${args[0]}" == "--" ]]; then
        args=("${args[@]:1}")
    fi
    use_local=1
    if [[ ${#args[@]} -gt 0 ]]; then
        for arg in "${args[@]}"; do
            if [[ "$arg" == "--aws" ]]; then
                use_local=0
            fi
        done
    fi
    if [[ "$use_local" -eq 1 ]]; then
        just dynamo
    fi
    if [[ ${#args[@]} -eq 0 ]]; then
        exec ./dqlrs-web/run.sh --dir "{{justfile_directory()}}/dqlrs-web/examples"
    fi
    exec ./dqlrs-web/run.sh "${args[@]}"

# Playwright tests for DQLRS Web. Flags need a `--` separator: just web-test -- --headed tests/editor.spec.js
[group('web')]
web-test *args: dynamo
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}/dqlrs-web/playwright"
    root="{{justfile_directory()}}"
    if [[ -z "${DQLRS_BIN:-}" ]]; then
        if [[ -x "$root/rust-impl/target/debug/dqlrs" ]]; then
            export DQLRS_BIN="$root/rust-impl/target/debug/dqlrs"
        elif [[ -x "$root/rust-impl/target/release/dqlrs" ]]; then
            export DQLRS_BIN="$root/rust-impl/target/release/dqlrs"
        fi
    fi
    if [[ ! -x node_modules/.bin/playwright ]]; then
        PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 npm ci
    fi
    # `just recipe -- --flag` includes the separator in the variadic args.
    args=({{args}})
    if [[ ${#args[@]} -gt 0 && "${args[0]}" == "--" ]]; then
        args=("${args[@]:1}")
    fi
    if [[ ${#args[@]} -gt 0 ]]; then
        ./node_modules/.bin/playwright test "${args[@]}"
    else
        ./node_modules/.bin/playwright test
    fi

# Playwright notebook tests marked slow. Headed, with 500ms between actions unless --slowmo is set.
[group('notebook')]
notebook-test-slow *args:
    #!/usr/bin/env bash
    set -euo pipefail
    # `just recipe -- --flag` includes the separator in the variadic args.
    args=({{args}})
    if [[ ${#args[@]} -gt 0 && "${args[0]}" == "--" ]]; then
        args=("${args[@]:1}")
    fi
    # --slowmo implies --headed. A later --slowmo in args replaces 500.
    if [[ ${#args[@]} -gt 0 ]]; then
        just notebook-test -- --slowmo 500 "${args[@]}" -m slow
    else
        just notebook-test -- --slowmo 500 -m slow
    fi

# Python, Rust, black-box, and Playwright notebook checks from VERIFICATION.md
[group('tests')]
verify: dynamo py rust black-box notebook-test

# Sync, lint, test, and build Python and Rust
prep: py rust

# Install dql and dqlrs onto ~/.local/bin
install: py-install rust-install
