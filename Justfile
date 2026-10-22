# Dev and maintainer tasks for the DQL repo. Run `just` from anywhere in the checkout.

# List dev tasks
default:
    @just --list

# Print the recipe name before its commands
_title name:
    @if [ -n "${NO_COLOR:-}" ]; then printf '\n[## %s ##]\n' "{{name}}"; else printf '\n\033[1;36m[## %s ##]\033[0m\n' "{{name}}"; fi

# Start DynamoDB Local in the background when port 8000 is down
[group('local')]
dynamo: (_title "dynamo")
    cd "{{justfile_directory()}}" && ./scripts/install_dynamodb_local.sh background

# List running DynamoDB Local processes and stop the ones you choose
[group('local')]
dynamo-stop: (_title "dynamo-stop")
    "{{justfile_directory()}}/scripts/stop_dynamodb_local.sh"

# Install Java 22 with sdkman for DynamoDB Local
[group('local')]
java: (_title "java")
    "{{justfile_directory()}}/scripts/install_sdkman_java.sh"

# Install Java 22, then start DynamoDB Local in the foreground
[group('local')]
local-install: (_title "local-install")
    "{{justfile_directory()}}/scripts/install.sh"

# Sync the Python dev environment
[group('python')]
py-sync: (_title "py-sync")
    cd "{{justfile_directory()}}/py-impl" && uv sync --dev

# Lint Python (mypy, isort, black, pylint)
[group('python')]
py-lint: (_title "py-lint")
    cd "{{justfile_directory()}}/py-impl" && uv run task lint

# Format Python with isort and black
[group('python')]
py-fix: (_title "py-fix")
    cd "{{justfile_directory()}}/py-impl" && uv run task fix

# Run Python tests (DynamoDB Local on port 8000)
[group('python')]
py-test: (_title "py-test")
    cd "{{justfile_directory()}}/py-impl" && uv run task test

# Run Python tests on 3.9, 3.10, and 3.11. Extra args go to pytest.
[group('python')]
py-test-matrix *args: (_title "py-test-matrix") dynamo
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    # `just recipe -- --flag` includes the separator in the variadic args.
    args=({{args}})
    if [[ ${#args[@]} -gt 0 && "${args[0]}" == "--" ]]; then
        args=("${args[@]:1}")
    fi
    if [[ ${#args[@]} -gt 0 ]]; then
        ./scripts/py-test-python-versions.sh "${args[@]}"
    else
        ./scripts/py-test-python-versions.sh
    fi

# Build the Python sdist and wheel
[group('python')]
py-build: (_title "py-build")
    cd "{{justfile_directory()}}/py-impl" && uv build

# Install dql as an editable uv tool (~/.local/bin/dql)
[group('python')]
py-install: (_title "py-install")
    cd "{{justfile_directory()}}/py-impl" && uv tool install --python 3.9 --editable --force .

# Sync, lint, test, and build the Python package
[group('python')]
py: (_title "py") py-sync py-lint py-test py-build

# Fetch Rust crates from Cargo.lock
[group('rust')]
rust-fetch: (_title "rust-fetch")
    cd "{{justfile_directory()}}/rust-impl" && cargo fetch --locked

# Check Rust formatting
[group('rust')]
rust-fmt: (_title "rust-fmt")
    cd "{{justfile_directory()}}/rust-impl" && cargo fmt --check

# Format Rust and apply Clippy suggestions
[group('rust')]
rust-fix: (_title "rust-fix")
    cd "{{justfile_directory()}}/rust-impl" && cargo fmt && cargo clippy --fix --workspace --allow-dirty --allow-staged

# Lint Rust with clippy
[group('rust')]
rust-clippy: (_title "rust-clippy")
    cd "{{justfile_directory()}}/rust-impl" && cargo clippy --workspace --all-targets -- -D warnings

# Run Rust workspace tests (DynamoDB Local on port 8000 for integration tests)
[group('rust')]
rust-test: (_title "rust-test")
    cd "{{justfile_directory()}}/rust-impl" && cargo test --workspace

# Run Rust DynamoDB Local smoke and parity tests
[group('rust')]
rust-local: (_title "rust-local")
    cd "{{justfile_directory()}}/rust-impl" && cargo test -p dql-engine --test dynamodb_local_smoke && cargo test -p dql-engine --test dynamodb_local_parity

# Build the release dqlrs binary
[group('rust')]
rust-build: (_title "rust-build")
    cd "{{justfile_directory()}}/rust-impl" && cargo build --release -p dql-cli

# Install dqlrs to ~/.local/bin
[group('rust')]
rust-install: (_title "rust-install")
    cd "{{justfile_directory()}}/rust-impl" && cargo install --path crates/dql-cli --locked --force --root "{{home_directory()}}/.local"

# Install the release dql binary from GitHub. Override with DQL_VERSION, DQL_INSTALL_DIR, or DQL_REPO.
[group('rust')]
rust-install-release: (_title "rust-install-release")
    "{{justfile_directory()}}/scripts/rust-install.sh"

# Smoke-test the release dqlrs binary (fails if DynamoDB Local is down)
[group('rust')]
rust-smoke: (_title "rust-smoke")
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    export DQL_BIN="{{justfile_directory()}}/rust-impl/target/release/dqlrs"
    export DQL_REQUIRE_LOCAL=1
    ./scripts/rust-smoke-test.sh

# Fetch, lint, test, build, and smoke-test Rust
[group('rust')]
rust: (_title "rust") rust-fetch rust-fmt rust-clippy rust-test rust-local rust-build rust-smoke

# Run black-box acceptance tests. Extra args are passed to black-box-tests/run.sh.
[group('py-rs')]
black-box *args: (_title "black-box")
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
notebook-dry-run: (_title "notebook-dry-run")
    cd "{{justfile_directory()}}" && ./notebook/start.sh --dry-run

# Start JupyterLab. Extra args are passed to notebook/start.sh.
[group('notebook')]
notebook *args: (_title "notebook")
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
notebook-test *args: (_title "notebook-test")
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

# MCP stdio server for the examples workspace. Point Cursor or Claude at `dqlrs --mcp` using dqlrs-web/mcp.example.json. This recipe is a manual session on that same folder.
[group('web')]
mcp: (_title "mcp") dynamo
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    (cd rust-impl && cargo build -p dql-cli)
    export AWS_ACCESS_KEY_ID="${AWS_ACCESS_KEY_ID:-fakeid}"
    export AWS_SECRET_ACCESS_KEY="${AWS_SECRET_ACCESS_KEY:-fakekey}"
    export AWS_REGION="${AWS_REGION:-us-west-1}"
    export AWS_EC2_METADATA_DISABLED="${AWS_EC2_METADATA_DISABLED:-true}"
    exec rust-impl/target/debug/dqlrs --mcp \
        --dir "{{justfile_directory()}}/dqlrs-web/examples" \
        -H "${DQL_HOST:-localhost}" \
        -p "${DQL_PORT:-8000}" \
        -r "$AWS_REGION"

# Stdio smoke test for the MCP server. Does not need DynamoDB.
[group('web')]
mcp-smoke: (_title "mcp-smoke")
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    (cd rust-impl && cargo build -p dql-cli)
    python3 scripts/mcp-smoke.py rust-impl/target/debug/dqlrs

# Unit tests and the stdio smoke test for the MCP server. Does not need DynamoDB.
[group('web')]
mcp-test: (_title "mcp-test") mcp-smoke
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    (cd rust-impl && cargo test -p dql-cli --lib mcp::)

# Start the web UI, API, and /mcp on one host. Extra args go to dqlrs-web/run.sh: just web -- --port 9000
[group('web')]
web *args: (_title "web")
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    (cd rust-impl && cargo build -p dql-cli)
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
    mcp=(--mcp)
    if [[ ${#args[@]} -gt 0 ]]; then
        for arg in "${args[@]}"; do
            if [[ "$arg" == "--mcp" ]]; then
                mcp=()
            fi
        done
    fi
    if [[ ${#args[@]} -eq 0 ]]; then
        exec ./dqlrs-web/run.sh --mcp --dir "{{justfile_directory()}}/dqlrs-web/examples"
    fi
    exec ./dqlrs-web/run.sh "${mcp[@]}" "${args[@]}"

# Node tests for the DQLRS Web editor
[group('web')]
web-unit: (_title "web-unit")
    node --test "{{justfile_directory()}}"/dqlrs-web/tests/*.mjs

# Playwright tests for DQLRS Web. Flags need a `--` separator: just web-test -- --headed tests/editor.spec.js
[group('web')]
web-test *args: (_title "web-test") dynamo
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{justfile_directory()}}"
    (cd "$root/rust-impl" && cargo build -p dql-cli)
    cd "$root/dqlrs-web/playwright"
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

# Playwright web tests, headed, with 500ms between actions unless --slow-mo is set.
[group('web')]
web-test-slow *args: (_title "web-test-slow")
    #!/usr/bin/env bash
    set -euo pipefail
    # `just recipe -- --flag` includes the separator in the variadic args.
    args=({{args}})
    if [[ ${#args[@]} -gt 0 && "${args[0]}" == "--" ]]; then
        args=("${args[@]:1}")
    fi
    # playwright test rejects --slow-mo. A later --slow-mo here replaces 500.
    slow_mo=500
    forwarded=()
    i=0
    while [[ $i -lt ${#args[@]} ]]; do
        arg="${args[$i]}"
        case "$arg" in
            --slow-mo)
                i=$((i + 1))
                if [[ $i -ge ${#args[@]} || -z "${args[$i]}" ]]; then
                    echo "error: --slow-mo requires milliseconds" >&2
                    exit 2
                fi
                slow_mo="${args[$i]}"
                ;;
            --slow-mo=*)
                slow_mo="${arg#--slow-mo=}"
                ;;
            *)
                forwarded+=("$arg")
                ;;
        esac
        i=$((i + 1))
    done
    if [[ ! "$slow_mo" =~ ^[0-9]+$ ]]; then
        echo "error: --slow-mo requires milliseconds" >&2
        exit 2
    fi
    export PLAYWRIGHT_SLOW_MO="$slow_mo"
    if [[ ${#forwarded[@]} -gt 0 ]]; then
        just web-test -- --headed "${forwarded[@]}"
    else
        just web-test -- --headed
    fi

# Playwright notebook tests marked slow. Headed, with 500ms between actions unless --slowmo is set.
[group('notebook')]
notebook-test-slow *args: (_title "notebook-test-slow")
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

# Bump Python or Rust and refresh that lockfile. Flags need `--`: just version -- rust --dry-run build
[group('other')]
version *args: (_title "version")
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    # `just recipe -- --flag` includes the separator in the variadic args.
    args=({{args}})
    if [[ ${#args[@]} -gt 0 && "${args[0]}" == "--" ]]; then
        args=("${args[@]:1}")
    fi
    if [[ ${#args[@]} -gt 0 ]]; then
        ./scripts/bump-version.sh "${args[@]}"
    else
        ./scripts/bump-version.sh
    fi

# Root recipes.

# Format Python and Rust and apply Clippy suggestions. Syncs the Python env first.
check: (_title "check") py-sync py-fix rust-fix

# Every test suite. Starts DynamoDB Local, syncs Python, and builds release dqlrs first.
test: (_title "test") dynamo py-sync rust-build web-unit py-test rust-test rust-local rust-smoke black-box notebook-test web-test mcp-test

# Static analysis, package builds, and every test suite. Does not call `test`.
verify: (_title "verify") dynamo web-unit py rust black-box notebook-test web-test mcp-smoke mcp-test

# Install dql and dqlrs onto ~/.local/bin
install: (_title "install") py rust py-install rust-install
