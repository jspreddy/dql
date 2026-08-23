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
Usage: run.sh [--bin dql|dqlrs|both] [--start-local] [--skip-teardown] [-v|--verbose] [filter]

Runs black-box acceptance cases under black-box-tests/cases/ against DynamoDB Local.

  --bin dql|dqlrs|both  Which binary to invoke (default: both, skipping missing)
  --start-local         Start Local only if the port is down (safe if already running)
  --skip-teardown       Do not run teardown.dql / default DROP TABLE after each case
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
SKIP_TEARDOWN=0
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
    --skip-teardown)
      SKIP_TEARDOWN=1
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
  local extra="${2:-}"
  local sgr="1"
  case "$status" in
    PASS) sgr="1;32" ;;
    FAIL) sgr="1;31" ;;
    SKIP) sgr="1;33" ;;
  esac
  printf '%s  %s' "$(harness_pad)" "$(harness_paint "$sgr" "$status")"
  if [[ -n "$extra" ]]; then
    printf '  %s' "$(harness_paint '1;37' "$extra")"
  fi
  printf '\n'
}

print_detail() {
  local sgr="$1"
  shift
  printf '%s      %s\n' "$(harness_pad)" "$(harness_paint "$sgr" "$*")"
}

print_detail_file() {
  local sgr="$1"
  local file="$2"
  if [[ ! -e "$file" ]]; then
    return 0
  fi
  if [[ ! -s "$file" ]]; then
    printf '%s        %s\n' "$(harness_pad)" "$(harness_paint 2 "(empty)")"
    return 0
  fi
  while IFS= read -r line || [[ -n "$line" ]]; do
    printf '%s        %s\n' "$(harness_pad)" "$(harness_paint "$sgr" "$line")"
  done <"$file"
}

harness_heading "black-box tests"
harness_kv "local" "$(local_host):$(local_port)"
harness_kv "bins" "$bin_list"
harness_kv "cases" "${#CASE_RELS[@]}"
if [[ -n "$FILTER" ]]; then
  harness_kv "filter" "$FILTER"
fi
if [[ "$HARNESS_VERBOSE" == "1" ]]; then
  harness_kv "verbose" "on"
fi
if [[ "$SKIP_TEARDOWN" == "1" ]]; then
  harness_kv "teardown" "skipped"
fi
harness_verbose "isolation $ISOLATION"
idx=0
for bin in "${BINS[@]}"; do
  harness_verbose "binary ${BIN_LABELS[$idx]} = $bin"
  idx=$((idx + 1))
done

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
  harness_heading "$rel"
  mode="oneshot"
  if [[ -f "$case_dir/mode" ]]; then
    mode="$(tr -d '[:space:]' <"$case_dir/mode" | tr '[:upper:]' '[:lower:]')"
  fi
  if [[ ! -f "$case_dir/expected.json" && ! -f "$case_dir/expected.stdout" ]]; then
    print_status FAIL
    printf '      missing %s or %s\n' "$(harness_filename expected.json)" "$(harness_filename expected.stdout)"
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

  harness_verbose "mode=$mode  json=$use_json  work=$work"
  harness_verbose_test_files "$case_dir"

  idx=0
  for bin in "${BINS[@]}"; do
    label="${BIN_LABELS[$idx]}"
    idx=$((idx + 1))

    harness_bin_heading "$label"
    HARNESS_INDENT=4

    if [[ "$mode" == "repl-stdin" && "$label" == "dqlrs" ]]; then
      print_status SKIP
      print_detail 2 "repl-stdin is Python dql only; dqlrs uses a TUI"
      SKIPPED=$((SKIPPED + 1))
      HARNESS_INDENT=0
      continue
    fi

    out="$work/stdout.$label"
    err="$work/stderr.$label"
    : >"$out"
    : >"$err"
    rc=0
    td_rc=""
    td_out=""
    td_err=""

    (
      trap - EXIT
      cd "$case_dir"
      if [[ "$mode" == "oneshot" ]]; then
        if [[ -f "$work/setup.dql" ]]; then
          setup_out="$work/setup.stdout.$label"
          setup_err="$work/setup.stderr.$label"
          setup_rc="$(run_cli "$bin" 0 "$(cat "$work/setup.dql")" "$setup_out" "$setup_err")"
          printf '%s' "$setup_rc" >"$work/setup.rc.$label"
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

    if [[ "$SKIP_TEARDOWN" != "1" && -s "$work/teardown.dql" ]]; then
      td_out="$work/teardown.stdout.$label"
      td_err="$work/teardown.stderr.$label"
      td_rc="$(
        trap - EXIT
        cd "$case_dir"
        run_cli "$bin" 0 "$(cat "$work/teardown.dql")" "$td_out" "$td_err"
      )" || true
    fi

    rc="$(cat "$work/rc.$label")"

    if [[ "$step" == "unknown-mode" ]]; then
      harness_verbose "notes: unknown mode '$mode'"
      print_status FAIL
      print_detail 31 "unknown mode '$mode'"
      FAILED=$((FAILED + 1))
      HARNESS_INDENT=0
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

    combined_note=""
    if [[ "$mode" == "file" || "$mode" == "repl-stdin" ]]; then
      combined_note="setup and input sent in one invocation"
    fi

    if [[ -f "$work/setup.dql" ]]; then
      setup_rc=0
      [[ -f "$work/setup.rc.$label" ]] && setup_rc="$(cat "$work/setup.rc.$label")"
      setup_notes=""
      if [[ "$step" == "setup" ]]; then
        setup_notes="setup failed"
      fi
      setup_out_arg=""
      setup_err_arg=""
      if [[ "$mode" == "oneshot" ]]; then
        setup_out_arg="$work/setup.stdout.$label"
        setup_err_arg="$work/setup.stderr.$label"
      fi
      harness_verbose_step "Setup" "setup.dql" "$work/setup.dql" "$setup_out_arg" "$setup_err_arg" "$setup_rc" "$setup_notes"
    fi

    if [[ "$step" != "setup" ]]; then
      input_notes=""
      if [[ "$mode" == "file" || "$mode" == "repl-stdin" ]]; then
        input_notes="$combined_note"
      fi
      harness_verbose_step "Input" "input.dql" "$work/input.dql" "$out" "$err" "$rc" "$input_notes"
    fi

    if [[ "$HARNESS_VERBOSE" == "1" ]]; then
      expect_names=""
      expect_count=0
      for expect_f in expected.json expected.stdout expected.stderr expected.exit; do
        if [[ -f "$work/$expect_f" || -f "$case_dir/$expect_f" ]]; then
          expect_count=$((expect_count + 1))
          if [[ -n "$expect_names" ]]; then
            expect_names="$expect_names, $(harness_filename "$expect_f")"
          else
            expect_names="$(harness_filename "$expect_f")"
          fi
        fi
      done
      if [[ -n "$expect_names" ]]; then
        printf '  %s %s %s\n' "$(harness_paint 2 "·")" "$(harness_paint '1;34' "Expect:")" "$expect_names"
        for expect_f in expected.json expected.stdout expected.stderr expected.exit; do
          expect_src=""
          if [[ -f "$work/$expect_f" ]]; then
            expect_src="$work/$expect_f"
          elif [[ -f "$case_dir/$expect_f" ]]; then
            expect_src="$case_dir/$expect_f"
          fi
          if [[ -n "$expect_src" ]]; then
            if [[ "$expect_count" -gt 1 ]]; then
              printf '%*s%s\n' 12 '' "$(harness_filename "$expect_f")"
            fi
            harness_verbose_body 12 "$expect_src"
          fi
        done
      fi
      if [[ "$cmp_rc" -ne 0 && -s "$work/cmp.err.$label" ]]; then
        harness_subheading "compare"
        print_detail_file 31 "$work/cmp.err.$label"
      fi
    fi

    if [[ "$SKIP_TEARDOWN" == "1" ]]; then
      harness_verbose_step "Teardown" "teardown.dql" "$work/teardown.dql" "" "" "0" "skipped (--skip-teardown)"
    elif [[ -s "$work/teardown.dql" ]]; then
      harness_verbose_step "Teardown" "teardown.dql" "$work/teardown.dql" "$td_out" "$td_err" "${td_rc:-0}" ""
    fi

    teardown_failed=0
    if [[ -n "$td_rc" && "$td_rc" != "0" ]]; then
      teardown_failed=1
    fi

    if [[ "$cmp_rc" -eq 0 && "$teardown_failed" -eq 0 ]]; then
      print_status PASS
      PASSED=$((PASSED + 1))
    else
      print_status FAIL
      FAILED=$((FAILED + 1))
      if [[ "$HARNESS_VERBOSE" != "1" ]]; then
        if [[ "$step" == "setup" ]]; then
          print_detail 31 "setup failed (exit $rc)"
        fi
        if [[ -s "$work/cmp.err.$label" ]]; then
          harness_subheading "compare"
          print_detail_file 31 "$work/cmp.err.$label"
        fi
        if [[ "$cmp_rc" -ne 0 ]]; then
          harness_subheading "stdout"
          print_detail_file 2 "$out"
          if [[ -s "$err" ]]; then
            harness_subheading "stderr"
            print_detail_file 31 "$err"
          fi
        fi
        if [[ "$teardown_failed" -eq 1 ]]; then
          print_detail 31 "teardown failed (exit $td_rc)"
          if [[ -n "$td_out" ]]; then
            harness_subheading "teardown stdout"
            print_detail_file 2 "$td_out"
          fi
          if [[ -n "$td_err" && -s "$td_err" ]]; then
            harness_subheading "teardown stderr"
            print_detail_file 31 "$td_err"
          fi
        fi
      else
        if [[ "$step" == "setup" ]]; then
          print_detail 31 "setup failed (exit $rc)"
        fi
        if [[ "$teardown_failed" -eq 1 ]]; then
          print_detail 31 "teardown failed (exit $td_rc)"
        fi
      fi
    fi
    HARNESS_INDENT=0
  done
done

harness_heading "summary"
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

if [[ "$FAILED" -ne 0 ]]; then
  exit 1
fi
if [[ "$PASSED" -eq 0 ]]; then
  harness_error "no cases passed"
  exit 1
fi
