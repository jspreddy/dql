#!/usr/bin/env bash
# Run `just verify` and print recipe headings, the exit code, and success or the failure.
set -u

root=$(git rev-parse --show-toplevel)
cd "$root"
log=${JUST_VERIFY_LOG:-/tmp/just-verify.log}

strip() {
  sed -E 's/\x1b\[[0-9;]*m//g' "$1"
}

headings() {
  strip "$1" | grep -E '^\[## .+ ##\]$' || true
}

exit_code() {
  strip "$1" | awk -F: '/^EXIT:/ { code = $2 } END { if (code == "") code = "missing"; print code }'
}

failure_lines() {
  strip "$1" | awk '
    /^\[## .+ ##\]$/ { start = NR }
    { line[NR] = $0 }
    /^error: recipe / { err = NR; err_heading = start }
    END {
      if (!err) {
        print "The log has no recipe error."
        exit 0
      }
      print line[err_heading]
      from = err_heading + 1
      if (err - 200 > from) from = err - 200
      n = 0
      for (i = from; i <= err; i++) {
        s = line[i]
        if (s ~ /attachment|Error Context|Usage:|show-trace|─/) continue
        if (s ~ /^  ✓/) continue
        if (s ~ /Error:|Expected|Received|^[[:space:]]*>|^[[:space:]]*[0-9]+\)|error: recipe|[0-9]+ failed|[0-9]+ passed/)
          keep[++n] = s
      }
      startn = n - 24
      if (startn < 1) startn = 1
      for (i = startn; i <= n; i++) print keep[i]
    }
  '
}

summarize() {
  headings "$1"
  code=$(exit_code "$1")
  printf 'exit: %s\n' "$code"
  printf 'log: %s\n' "$1"
  if [[ $code == 0 ]]; then
    printf 'The verify suite was successful.\n'
  else
    printf 'The verify suite failed.\n'
    failure_lines "$1"
  fi
}

failed() {
  if [[ ! -f $log ]]; then
    printf 'exit: missing\nlog: %s\nThe verify suite failed.\nThe log is missing.\n' "$log"
    exit 1
  fi
  code=$(exit_code "$log")
  if [[ $code == 0 ]]; then
    printf 'exit: 0\nlog: %s\nThe verify suite was successful.\n' "$log"
    return 0
  fi
  printf 'exit: %s\nlog: %s\nThe verify suite failed.\n' "$code" "$log"
  failure_lines "$log"
}

case ${1:-} in
  --from-log)
    if [[ ! -f $log ]]; then
      printf 'exit: missing\nlog: %s\nThe verify suite failed.\nThe log is missing.\n' "$log"
      exit 1
    fi
    summarize "$log"
    code=$(exit_code "$log")
    [[ $code == "missing" ]] && exit 1
    exit "$code"
    ;;
  --failed)
    failed
    code=$(exit_code "$log")
    [[ $code == "missing" ]] && exit 1
    exit "$code"
    ;;
  "")
    ;;
  *)
    printf 'usage: run.sh [--from-log|--failed]\n' >&2
    exit 2
    ;;
esac

just verify >"$log" 2>&1
code=$?
printf 'EXIT:%s\n' "$code" >>"$log"
summarize "$log"
exit "$code"
