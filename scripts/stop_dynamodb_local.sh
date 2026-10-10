#!/bin/bash
# List running DynamoDB Local processes and stop the ones the user selects.
# Compatible with bash 3.2+ (macOS /bin/bash).
set -euo pipefail

pids=()
ports=()
labels=()

trim() {
  local value="$1"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "$value"
}

listen_port() {
  local pid="$1"
  local line port
  line=$(lsof -nP -a -p "$pid" -iTCP -sTCP:LISTEN 2>/dev/null | awk 'NR>1 { print $9; exit }')
  port="${line##*:}"
  if [[ "$port" =~ ^[0-9]+$ ]]; then
    printf '%s' "$port"
  else
    printf '?'
  fi
}

if ! listing=$(ps -ax -o pid= -o command=); then
  echo "Could not list processes." >&2
  exit 1
fi

while IFS= read -r line; do
  line=$(trim "$line")
  [[ -n "$line" ]] || continue
  pid="${line%% *}"
  cmd="${line#"$pid"}"
  cmd=$(trim "$cmd")
  case "$cmd" in
    *DynamoDBLocal.jar*) ;;
    *) continue ;;
  esac
  label="${cmd#*DynamoDBLocal.jar}"
  label=$(trim "DynamoDBLocal.jar${label}")
  pids+=("$pid")
  ports+=("$(listen_port "$pid")")
  labels+=("$label")
done <<<"$listing"

count=${#pids[@]}
if [[ "$count" -eq 0 ]]; then
  echo "No DynamoDB Local servers are running."
  exit 0
fi

echo "DynamoDB Local servers:"
i=1
while [[ "$i" -le "$count" ]]; do
  idx=$((i - 1))
  printf '  %s) pid %s  port %s  %s\n' "$i" "${pids[$idx]}" "${ports[$idx]}" "${labels[$idx]}"
  i=$((i + 1))
done

if [[ ! -t 0 ]]; then
  echo "Run \`just dynamo-stop\` in a terminal to choose which servers to stop." >&2
  exit 1
fi

printf "Stop which servers? Numbers (1,2), 'a' for all, or Enter to cancel: "
IFS= read -r answer || answer=""
answer=$(trim "$answer")
if [[ -z "$answer" ]]; then
  echo "Nothing selected."
  exit 0
fi

lower=$(printf '%s' "$answer" | tr '[:upper:]' '[:lower:]')
selected=()
if [[ "$lower" == "a" || "$lower" == "all" ]]; then
  i=0
  while [[ "$i" -lt "$count" ]]; do
    selected+=("$i")
    i=$((i + 1))
  done
else
  normalized=${answer//,/ }
  for part in $normalized; do
    if [[ ! "$part" =~ ^[0-9]+$ ]] || [[ "$part" -lt 1 ]] || [[ "$part" -gt "$count" ]]; then
      echo "Choose a number from 1 to $count, 'a' for all, or press Enter to cancel." >&2
      exit 1
    fi
    idx=$((part - 1))
    already=0
    for seen in "${selected[@]+"${selected[@]}"}"; do
      if [[ "$seen" == "$idx" ]]; then
        already=1
        break
      fi
    done
    if [[ "$already" -eq 0 ]]; then
      selected+=("$idx")
    fi
  done
fi

if [[ ${#selected[@]} -eq 0 ]]; then
  echo "Nothing selected."
  exit 0
fi

failed=0
for idx in "${selected[@]}"; do
  pid="${pids[$idx]}"
  echo "Stopping DynamoDB Local pid $pid (port ${ports[$idx]})"
  if ! kill "$pid"; then
    echo "Could not stop pid $pid" >&2
    failed=1
  fi
done

exit "$failed"
