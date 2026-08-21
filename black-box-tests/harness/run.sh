#!/usr/bin/env bash
# Black-box acceptance runner: spawn dql / dqlrs against DynamoDB Local.
# Written for bash 3.2+ (macOS /bin/bash) as well as bash 4+.
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
Usage: run.sh [--bin dql|dqlrs|both] [--start-local] [-v|--verbose] [filter]

Runs black-box acceptance cases under black-box-tests/cases/ against DynamoDB Local.

  --bin dql|dqlrs|both  Which binary to invoke (default: both, skipping missing)
  --start-local         Start Local only if the port is down (safe if already running)
  -v, --verbose         Log setup, commands, DQL, and CLI output for each case
  filter                Numeric group (1xx, 11x, 2xx) or substring of the case folder name

Environment:
  DQL_BIN / DQLRS_BIN         Explicit binary paths
  DQL_LOCAL_HOST / DQL_LOCAL_PORT   Default localhost:8000
  AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY   Dummy keys if unset
  NO_COLOR                    Set to disable ANSI color
EOF
}

BIN_CHOICE="both"
START_LOCAL=0
FILTER=""
HARNESS_VERBOSE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    --bin)
      BIN_CHOICE="${2:-}"
      if [[ -z "$BIN_CHOICE" ]]; then
        harness_error "--bin requires dql, dqlrs, or both"
        exit 2
      fi
      shift 2
      ;;
    --start-local)
      START_LOCAL=1
      shift
      ;;
    -v | --verbose)
      HARNESS_VERBOSE=1
      shift
      ;;
    --)
      shift
      break
      ;;
    -*)
      harness_error "unknown flag $1"
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
    harness_error "--bin must be dql, dqlrs, or both (got $BIN_CHOICE)"
    exit 2
    ;;
esac

export AWS_ACCESS_KEY_ID="${AWS_ACCESS_KEY_ID:-fakeid}"
export AWS_SECRET_ACCESS_KEY="${AWS_SECRET_ACCESS_KEY:-fakekey}"
export AWS_DEFAULT_REGION="${AWS_DEFAULT_REGION:-us-west-1}"
export AWS_REGION="${AWS_REGION:-us-west-1}"
unset DQL_BACKEND || true

if [[ "$START_LOCAL" == "1" ]]; then
  harness_verbose "ensuring DynamoDB Local at $(local_host):$(local_port)"
  if ! start_local; then
    exit 1
  fi
else
  harness_verbose "checking DynamoDB Local at $(local_host):$(local_port)"
  if local_available; then
    harness_info "DynamoDB Local already running at $(local_host):$(local_port)"
  else
    require_local
  fi
fi

HOST="$(local_host)"
PORT="$(local_port)"

if ! python3 "$COMPARE_PY" --self-test; then
  harness_error "harness compare.py self-test failed"
  exit 1
fi
harness_verbose "compare.py self-test passed"

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
    harness_error "dql not found (set DQL_BIN or install dql on PATH)"
    exit 1
  fi
  BINS+=("$DQL_PATH")
  BIN_LABELS+=("dql")
elif [[ "$BIN_CHOICE" == "dqlrs" ]]; then
  if [[ -z "$DQLRS_PATH" ]]; then
    harness_error "dqlrs not found (set DQLRS_BIN or install dqlrs on PATH)"
    exit 1
  fi
  BINS+=("$DQLRS_PATH")
  BIN_LABELS+=("dqlrs")
else
  if [[ -n "$DQL_PATH" ]]; then
    BINS+=("$DQL_PATH")
    BIN_LABELS+=("dql")
  else
    harness_warn "dql not found; skipping (set DQL_BIN to require it)"
  fi
  if [[ -n "$DQLRS_PATH" ]]; then
    BINS+=("$DQLRS_PATH")
    BIN_LABELS+=("dqlrs")
  else
    harness_warn "dqlrs not found; skipping (set DQLRS_BIN to require it)"
  fi
  if [[ ${#BINS[@]} -eq 0 ]]; then
    harness_error "no dql or dqlrs binary found"
    exit 1
  fi
fi

CASE_RELS=()
while IFS= read -r _rel || [[ -n "${_rel:-}" ]]; do
  [[ -z "${_rel:-}" ]] && continue
  CASE_RELS+=("$_rel")
done < <(list_case_dirs "$CASES_ROOT" "$FILTER")
if [[ ${#CASE_RELS[@]} -eq 0 ]]; then
  harness_error "no cases with input.dql under $CASES_ROOT${FILTER:+ matching '$FILTER'}"
  exit 1
fi

ISOLATION="$(mktemp -d "${TMPDIR:-/tmp}/dql-at-home.XXXXXX")"
export HOME="$ISOLATION/home"
export XDG_CONFIG_HOME="$HOME/.config"
mkdir -p "$XDG_CONFIG_HOME" "$HOME/.dql"
cleanup_isolation() {
  rm -rf "$ISOLATION"
}
trap cleanup_isolation EXIT
harness_verbose "isolation $ISOLATION"

idx=0
for bin in "${BINS[@]}"; do
  harness_verbose "binary ${BIN_LABELS[$idx]} = $bin"
  idx=$((idx + 1))
done

NAME_WIDTH=8
for rel in "${CASE_RELS[@]}"; do
  if [[ ${#rel} -gt $NAME_WIDTH ]]; then
    NAME_WIDTH=${#rel}
  fi
done

bin_list=""
for label in "${BIN_LABELS[@]}"; do
  if [[ -n "$bin_list" ]]; then
    bin_list="$bin_list, $label"
  else
    bin_list="$label"
  fi
done

print_status() {
  local status="$1"
  local rel="$2"
  local extra="${3:-}"
  local sgr="1"
  case "$status" in
    PASS) sgr="1;32" ;;
    FAIL) sgr="1;31" ;;
    SKIP) sgr="1;33" ;;
  esac
  printf '  %s  %s' "$(harness_paint "$sgr" "$status")" "$(printf '%-*s' "$NAME_WIDTH" "$rel")"
  if [[ -n "$extra" ]]; then
    printf '  %s' "$(harness_paint 2 "$extra")"
  fi
  printf '\n'
}

print_detail() {
  local sgr="$1"
  shift
  printf '      %s\n' "$(harness_paint "$sgr" "$*")"
}

print_detail_file() {
  local sgr="$1"
  local file="$2"
  if [[ ! -e "$file" ]]; then
    return 0
  fi
  if [[ ! -s "$file" ]]; then
    printf '        %s\n' "$(harness_paint 2 "(empty)")"
    return 0
  fi
  while IFS= read -r line || [[ -n "$line" ]]; do
    printf '        %s\n' "$(harness_paint "$sgr" "$line")"
  done <"$file"
}

harness_hr
printf '  %s\n' "$(harness_paint 1 "black-box tests")"
harness_kv "local" "$(local_host):$(local_port)"
harness_kv "bins" "$bin_list"
harness_kv "cases" "${#CASE_RELS[@]}"
if [[ -n "$FILTER" ]]; then
  harness_kv "filter" "$FILTER"
fi
if [[ "$HARNESS_VERBOSE" == "1" ]]; then
  harness_kv "verbose" "on"
fi
harness_hr

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
    print_status FAIL "$rel"
    print_detail 31 "missing expected.json or expected.stdout"
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

  harness_verbose "$rel  mode=$mode  json=$use_json  work=$work"
  harness_verbose_file "setup.dql" "$work/setup.dql"
  harness_verbose_file "input.dql" "$work/input.dql"
  harness_verbose_file "teardown.dql" "$work/teardown.dql"
  harness_verbose_file "expected.json" "$work/expected.json"
  harness_verbose_file "expected.stdout" "$work/expected.stdout"

  idx=0
  for bin in "${BINS[@]}"; do
    label="${BIN_LABELS[$idx]}"
    idx=$((idx + 1))

    if [[ "$mode" == "repl-stdin" && "$label" == "dqlrs" ]]; then
      print_status SKIP "$rel" "$label"
      print_detail 2 "repl-stdin is Python dql only; dqlrs uses a TUI"
      SKIPPED=$((SKIPPED + 1))
      continue
    fi

    json_note=""
    if [[ "$use_json" == "1" ]]; then
      json_note=" --json"
    fi
    harness_verbose "[$label] exec $bin -H $HOST -p $PORT -r ${AWS_REGION:-us-west-1}$json_note"

    out="$work/stdout.$label"
    err="$work/stderr.$label"
    : >"$out"
    : >"$err"
    rc=0

    (
      trap - EXIT
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
            printf 'setup' >"$work/step.$label"
            exit 0
          fi
        fi
        rc="$(run_cli "$bin" "$use_json" "$(cat "$work/input.dql")" "$out" "$err")"
        printf '%s' "$rc" >"$work/rc.$label"
        printf 'input' >"$work/step.$label"
      elif [[ "$mode" == "file" ]]; then
        {
          [[ -f "$work/setup.dql" ]] && cat "$work/setup.dql"
          cat "$work/input.dql"
        } >"$work/all.dql"
        rc="$(run_cli "$bin" "$use_json" "file $work/all.dql" "$out" "$err")"
        printf '%s' "$rc" >"$work/rc.$label"
        printf 'file' >"$work/step.$label"
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
        printf 'repl-stdin' >"$work/step.$label"
      else
        printf '99' >"$work/rc.$label"
        printf 'unknown-mode' >"$work/step.$label"
      fi
    )

    step="input"
    [[ -f "$work/step.$label" ]] && step="$(cat "$work/step.$label")"
    harness_verbose "step $step"
    if [[ -f "$work/setup.stdout.$label" ]]; then
      harness_verbose_file "setup stdout" "$work/setup.stdout.$label"
      harness_verbose_file "setup stderr" "$work/setup.stderr.$label"
    fi
    if [[ -f "$work/all.dql" ]]; then
      harness_verbose_file "file $work/all.dql" "$work/all.dql"
    fi

    # Always teardown so Local does not accumulate tables.
    if [[ -s "$work/teardown.dql" ]]; then
      td_out="$work/teardown.stdout.$label"
      td_err="$work/teardown.stderr.$label"
      (
        trap - EXIT
        cd "$case_dir"
        run_cli "$bin" 0 "$(cat "$work/teardown.dql")" "$td_out" "$td_err" >/dev/null || true
      )
      harness_verbose_file "teardown stdout" "$td_out"
      harness_verbose_file "teardown stderr" "$td_err"
    fi

    rc="$(cat "$work/rc.$label")"
    harness_verbose "asserted command exit $rc (expected $expected_exit)"
    harness_verbose_file "stdout" "$out"
    harness_verbose_file "stderr" "$err"

    if [[ "$step" == "unknown-mode" ]]; then
      print_status FAIL "$rel" "$label"
      print_detail 31 "unknown mode '$mode'"
      FAILED=$((FAILED + 1))
      continue
    fi

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
      print_status PASS "$rel" "$label"
      PASSED=$((PASSED + 1))
    else
      print_status FAIL "$rel" "$label"
      FAILED=$((FAILED + 1))
      if [[ "$step" == "setup" ]]; then
        print_detail 31 "setup failed (exit $rc)"
      fi
      if [[ -s "$work/cmp.err.$label" ]]; then
        print_detail 2 "compare"
        print_detail_file 31 "$work/cmp.err.$label"
      fi
      print_detail 2 "stdout"
      print_detail_file 2 "$out"
      if [[ -s "$err" ]]; then
        print_detail 2 "stderr"
        print_detail_file 31 "$err"
      fi
    fi
  done
done

harness_hr
pass_txt="$(harness_paint '1;32' "$PASSED passed")"
if [[ "$FAILED" -gt 0 ]]; then
  fail_txt="$(harness_paint '1;31' "$FAILED failed")"
else
  fail_txt="$(harness_paint 2 "$FAILED failed")"
fi
if [[ "$SKIPPED" -gt 0 ]]; then
  skip_txt="$(harness_paint '1;33' "$SKIPPED skipped")"
else
  skip_txt="$(harness_paint 2 "$SKIPPED skipped")"
fi
printf '  %s    %s    %s\n' "$pass_txt" "$fail_txt" "$skip_txt"
harness_hr

if [[ "$FAILED" -ne 0 ]]; then
  exit 1
fi
if [[ "$PASSED" -eq 0 ]]; then
  harness_error "no cases passed"
  exit 1
fi
