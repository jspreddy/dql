#!/usr/bin/env bash
# Black-box acceptance runner: spawn dql / dqlrs against DynamoDB Local.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

SUITE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CASES_ROOT="$SUITE_ROOT/cases"
REPO_ROOT="$(cd "$SUITE_ROOT/.." && pwd)"
COMPARE_PY="$SCRIPT_DIR/compare.py"

usage() {
  cat <<'EOF'
Usage: run.sh [--bin dql|dqlrs|both] [--start-local] [filter]

Runs black-box acceptance cases under black-box-tests/cases/ against DynamoDB Local.

  --bin dql|dqlrs|both  Which binary to invoke (default: both, skipping missing)
  --start-local         Start ./scripts/install_dynamodb_local.sh if port is down
  filter                Substring match on cases/<family>/<slug>

Environment:
  DQL_BIN / DQLRS_BIN         Explicit binary paths
  DQL_LOCAL_HOST / DQL_LOCAL_PORT   Default localhost:8000
  AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY   Dummy keys if unset
EOF
}

BIN_CHOICE="both"
START_LOCAL=0
FILTER=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    --bin)
      BIN_CHOICE="${2:-}"
      if [[ -z "$BIN_CHOICE" ]]; then
        echo "error: --bin requires dql, dqlrs, or both" >&2
        exit 2
      fi
      shift 2
      ;;
    --start-local)
      START_LOCAL=1
      shift
      ;;
    --)
      shift
      break
      ;;
    -*)
      echo "error: unknown flag $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      FILTER="$1"
      shift
      ;;
  esac
done

if [[ $# -gt 0 && -z "$FILTER" ]]; then
  FILTER="$1"
fi

case "$BIN_CHOICE" in
  dql | dqlrs | both) ;;
  *)
    echo "error: --bin must be dql, dqlrs, or both (got $BIN_CHOICE)" >&2
    exit 2
    ;;
esac

export AWS_ACCESS_KEY_ID="${AWS_ACCESS_KEY_ID:-fakeid}"
export AWS_SECRET_ACCESS_KEY="${AWS_SECRET_ACCESS_KEY:-fakekey}"
export AWS_DEFAULT_REGION="${AWS_DEFAULT_REGION:-us-west-1}"
export AWS_REGION="${AWS_REGION:-us-west-1}"
unset DQL_BACKEND || true

if [[ "$START_LOCAL" == "1" ]]; then
  if ! start_local; then
    echo "error: failed to start DynamoDB Local" >&2
    exit 1
  fi
else
  require_local
fi

HOST="$(local_host)"
PORT="$(local_port)"

if ! python3 "$COMPARE_PY" --self-test; then
  echo "error: harness compare.py self-test failed" >&2
  exit 1
fi

DQL_PATH=""
DQLRS_PATH=""
if [[ "$BIN_CHOICE" == "dql" || "$BIN_CHOICE" == "both" ]]; then
  DQL_PATH="$(find_named_bin dql DQL_BIN)"
fi
if [[ "$BIN_CHOICE" == "dqlrs" || "$BIN_CHOICE" == "both" ]]; then
  DQLRS_PATH="$(find_named_bin dqlrs DQLRS_BIN)"
fi

BINS=()
BIN_LABELS=()
if [[ "$BIN_CHOICE" == "dql" ]]; then
  if [[ -z "$DQL_PATH" ]]; then
    echo "error: dql not found (set DQL_BIN or install dql on PATH)" >&2
    exit 1
  fi
  BINS+=("$DQL_PATH")
  BIN_LABELS+=("dql")
elif [[ "$BIN_CHOICE" == "dqlrs" ]]; then
  if [[ -z "$DQLRS_PATH" ]]; then
    echo "error: dqlrs not found (set DQLRS_BIN or install dqlrs on PATH)" >&2
    exit 1
  fi
  BINS+=("$DQLRS_PATH")
  BIN_LABELS+=("dqlrs")
else
  if [[ -n "$DQL_PATH" ]]; then
    BINS+=("$DQL_PATH")
    BIN_LABELS+=("dql")
  else
    echo "warning: dql not found; skipping (set DQL_BIN to require it)" >&2
  fi
  if [[ -n "$DQLRS_PATH" ]]; then
    BINS+=("$DQLRS_PATH")
    BIN_LABELS+=("dqlrs")
  else
    echo "warning: dqlrs not found; skipping (set DQLRS_BIN to require it)" >&2
  fi
  if [[ ${#BINS[@]} -eq 0 ]]; then
    echo "error: no dql or dqlrs binary found" >&2
    exit 1
  fi
fi

mapfile -t CASE_RELS < <(list_case_dirs "$CASES_ROOT" "$FILTER")
if [[ ${#CASE_RELS[@]} -eq 0 ]]; then
  echo "error: no cases with input.dql under $CASES_ROOT${FILTER:+ matching '$FILTER'}" >&2
  exit 1
fi

ISOLATION="$(mktemp -d "${TMPDIR:-/tmp}/dql-at-home.XXXXXX")"
export HOME="$ISOLATION/home"
export XDG_CONFIG_HOME="$HOME/.config"
mkdir -p "$XDG_CONFIG_HOME" "$HOME/.dql"
cleanup_isolation() {
  # Pipeline subshells inherit EXIT; only the top-level shell should remove the dir.
  if [[ "${BASH_SUBSHELL:-0}" -gt 0 ]]; then
    return 0
  fi
  rm -rf "$ISOLATION"
}
trap cleanup_isolation EXIT

run_cli() {
  local bin="$1"
  local json_flag="$2"
  local command="$3"
  local out_file="$4"
  local err_file="$5"
  local args=("-H" "$HOST" "-p" "$PORT" "-r" "${AWS_REGION:-us-west-1}")
  if [[ "$json_flag" == "1" ]]; then
    args+=(--json)
  fi
  args+=(-c "$command")
  set +e
  "$bin" "${args[@]}" >"$out_file" 2>"$err_file"
  local rc=$?
  set -e
  printf '%s' "$rc"
}

default_teardown() {
  local slug="$1"
  local source_text="$2"
  local indexes
  indexes="$(printf '%s' "$source_text" | used_table_indexes)"
  if [[ -z "$indexes" ]]; then
    indexes="1"
  fi
  local n
  for n in $indexes; do
    if [[ "$n" == "1" ]]; then
      printf 'DROP TABLE {{TABLE}};\n'
    else
      printf 'DROP TABLE {{TABLE%s}};\n' "$n"
    fi
  done
}

PASSED=0
FAILED=0
SKIPPED=0

for rel in "${CASE_RELS[@]}"; do
  case_dir="$CASES_ROOT/$rel"
  mode="oneshot"
  if [[ -f "$case_dir/mode" ]]; then
    mode="$(tr -d '[:space:]' <"$case_dir/mode" | tr '[:upper:]' '[:lower:]')"
  fi
  if [[ ! -f "$case_dir/expected.json" && ! -f "$case_dir/expected.stdout" ]]; then
    echo "FAIL  $rel (missing expected.json or expected.stdout)"
    FAILED=$((FAILED + 1))
    continue
  fi

  source_blob=""
  [[ -f "$case_dir/setup.dql" ]] && source_blob+="$(cat "$case_dir/setup.dql")"$'\n'
  source_blob+="$(cat "$case_dir/input.dql")"$'\n'
  [[ -f "$case_dir/teardown.dql" ]] && source_blob+="$(cat "$case_dir/teardown.dql")"$'\n'

  work="$(mktemp -d "$ISOLATION/case.XXXXXX")"
  if [[ -f "$case_dir/setup.dql" ]]; then
    apply_tables "$rel" <"$case_dir/setup.dql" >"$work/setup.dql"
  fi
  apply_tables "$rel" <"$case_dir/input.dql" >"$work/input.dql"
  if [[ -f "$case_dir/teardown.dql" ]]; then
    apply_tables "$rel" <"$case_dir/teardown.dql" >"$work/teardown.dql"
  else
    default_teardown "$rel" "$source_blob" >"$work/teardown.raw"
    apply_tables "$rel" <"$work/teardown.raw" >"$work/teardown.dql"
  fi
  if [[ -f "$case_dir/expected.json" ]]; then
    apply_tables "$rel" <"$case_dir/expected.json" >"$work/expected.json"
  fi
  if [[ -f "$case_dir/expected.stdout" ]]; then
    apply_tables "$rel" <"$case_dir/expected.stdout" >"$work/expected.stdout"
  fi
  if [[ -f "$case_dir/expected.stderr" ]]; then
    apply_tables "$rel" <"$case_dir/expected.stderr" >"$work/expected.stderr"
  fi
  expected_exit=0
  if [[ -f "$case_dir/expected.exit" ]]; then
    expected_exit="$(tr -d '[:space:]' <"$case_dir/expected.exit")"
  fi

  use_json=0
  if [[ -f "$case_dir/expected.json" ]]; then
    use_json=1
  fi

  idx=0
  for bin in "${BINS[@]}"; do
    label="${BIN_LABELS[$idx]}"
    idx=$((idx + 1))
    tag="$rel [$label]"

    if [[ "$mode" == "repl-stdin" && "$label" == "dqlrs" ]]; then
      echo "SKIP  $tag (repl-stdin is Python dql only; dqlrs uses a TUI)"
      SKIPPED=$((SKIPPED + 1))
      continue
    fi

    out="$work/stdout.$label"
    err="$work/stderr.$label"
    : >"$out"
    : >"$err"
    rc=0

    (
      cd "$case_dir"
      if [[ "$mode" == "oneshot" ]]; then
        if [[ -f "$work/setup.dql" ]]; then
          setup_out="$work/setup.stdout.$label"
          setup_err="$work/setup.stderr.$label"
          setup_rc="$(run_cli "$bin" 0 "$(cat "$work/setup.dql")" "$setup_out" "$setup_err")"
          if [[ "$setup_rc" != "0" ]]; then
            cat "$setup_out" >"$out"
            cat "$setup_err" >"$err"
            printf '%s' "$setup_rc" >"$work/rc.$label"
            exit 0
          fi
        fi
        rc="$(run_cli "$bin" "$use_json" "$(cat "$work/input.dql")" "$out" "$err")"
        printf '%s' "$rc" >"$work/rc.$label"
      elif [[ "$mode" == "file" ]]; then
        {
          [[ -f "$work/setup.dql" ]] && cat "$work/setup.dql"
          cat "$work/input.dql"
        } >"$work/all.dql"
        rc="$(run_cli "$bin" "$use_json" "file $work/all.dql" "$out" "$err")"
        printf '%s' "$rc" >"$work/rc.$label"
      elif [[ "$mode" == "repl-stdin" ]]; then
        set +e
        {
          [[ -f "$work/setup.dql" ]] && cat "$work/setup.dql"
          cat "$work/input.dql"
          printf 'exit\n'
        } | "$bin" -H "$HOST" -p "$PORT" -r "${AWS_REGION:-us-west-1}" >"$out" 2>"$err"
        rc=$?
        set -e
        printf '%s' "$rc" >"$work/rc.$label"
      else
        echo "FAIL  $tag (unknown mode '$mode')" >&2
        printf '99' >"$work/rc.$label"
      fi
    )

    # Always teardown so Local does not accumulate tables.
    if [[ -s "$work/teardown.dql" ]]; then
      td_out="$work/teardown.stdout.$label"
      td_err="$work/teardown.stderr.$label"
      (
        cd "$case_dir"
        run_cli "$bin" 0 "$(cat "$work/teardown.dql")" "$td_out" "$td_err" >/dev/null || true
      )
    fi

    rc="$(cat "$work/rc.$label")"
    cmp_args=(
      --stdout-file "$out"
      --stderr-file "$err"
      --exit "$rc"
      --expected-exit "$expected_exit"
    )
    [[ -f "$work/expected.json" ]] && cmp_args+=(--expected-json "$work/expected.json")
    [[ -f "$work/expected.stdout" ]] && cmp_args+=(--expected-stdout "$work/expected.stdout")
    [[ -f "$work/expected.stderr" ]] && cmp_args+=(--expected-stderr "$work/expected.stderr")

    set +e
    python3 "$COMPARE_PY" "${cmp_args[@]}" >"$work/cmp.out.$label" 2>"$work/cmp.err.$label"
    cmp_rc=$?
    set -e
    if [[ "$cmp_rc" -eq 0 ]]; then
      echo "PASS  $tag"
      PASSED=$((PASSED + 1))
    else
      echo "FAIL  $tag"
      FAILED=$((FAILED + 1))
      if [[ -s "$work/cmp.err.$label" ]]; then
        sed 's/^/      /' "$work/cmp.err.$label"
      fi
      echo "      stdout:"
      sed 's/^/        /' "$out" || true
      if [[ -s "$err" ]]; then
        echo "      stderr:"
        sed 's/^/        /' "$err"
      fi
    fi
  done
done

echo
echo "passed=$PASSED failed=$FAILED skipped=$SKIPPED"
if [[ "$FAILED" -ne 0 ]]; then
  exit 1
fi
if [[ "$PASSED" -eq 0 ]]; then
  echo "error: no cases passed" >&2
  exit 1
fi
