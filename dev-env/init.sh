#!/usr/bin/env bash
# Set up a maintainer machine for this repository: the toolchains, test browsers, and agent CLIs
# that `just verify` and the agents use. It is for maintainers, not for people who install dql.
#
# One script for every place we develop:
#   - the dev-env Dockerfile (Docker sandbox template), with --no-repo at build time
#   - Claude Code cloud environments (setup script) and Cursor cloud agents (install command)
#   - a fresh Ubuntu or Debian machine or WSL
#
# Every step checks what is already there and installs only what is missing, so running it again
# is cheap. Run it as root, or as a user with sudo; tools that live in a home directory (rustup,
# uv, just, claude, cursor-agent) go to the user that runs it.
#
#   dev-env/init.sh                 every step; the repo step runs when the script is in a checkout
#   dev-env/init.sh --no-repo       tools only (image builds)
#   dev-env/init.sh rust browser    only the named steps
#   dev-env/init.sh --skip agents   every step except agents
#   dev-env/init.sh --check         report what is installed and change nothing
#
# Steps, in order:
#   system      apt packages: build tools, cmake (aws-lc-sys), Java (DynamoDB Local), git, jq, gh
#   node        Node.js 22 when node is missing or older than 20
#   rust        rustup with the stable toolchain, rustfmt, and clippy (rust-impl/rust-toolchain.toml)
#   tools       uv and just; cargo-release (`just version`) and git-graph (the git-graph skill)
#   agents      Claude Code and the Cursor CLI (cursor-agent)
#   browser     Google Chrome for the Playwright tests (they use channel "chrome")
#   repo        Python envs, cargo crates, Playwright packages, and the DynamoDB Local download
#
# Environment:
#   DQL_REPO_DIR       checkout for the repo step (default: the checkout holding this script)
#   DQL_SKIP_CARGO_TOOLS=1   skip cargo-release and git-graph; they compile from source and are slow
set -euo pipefail

STEPS_ALL=(system node rust tools agents browser repo)
NODE_MAJOR=22
NODE_MIN=20

log() { printf '\n[dev-env] %s\n' "$*"; }
note() { printf '[dev-env]   %s\n' "$*"; }
have() { command -v "$1" >/dev/null 2>&1; }

usage() {
  sed -n '2,/^set -euo/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//'
}

# --- arguments --------------------------------------------------------------------------------

selected=()
skipped=()
check_only=0
no_repo=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h | --help) usage; exit 0 ;;
    --check) check_only=1 ;;
    --no-repo) no_repo=1 ;;
    --skip)
      [[ $# -ge 2 ]] || { echo "--skip needs a step" >&2; exit 2; }
      skipped+=("$2"); shift ;;
    system | node | rust | tools | agents | browser | repo) selected+=("$1") ;;
    *) echo "unknown argument: $1 (see --help)" >&2; exit 2 ;;
  esac
  shift
done
[[ ${#selected[@]} -gt 0 ]] || selected=("${STEPS_ALL[@]}")

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="${DQL_REPO_DIR:-}"
if [[ -z "$repo_dir" && -f "$script_dir/../Justfile" && -d "$script_dir/../rust-impl" ]]; then
  repo_dir="$(cd "$script_dir/.." && pwd)"
fi

wanted() {
  local step="$1" s
  for s in "${skipped[@]+"${skipped[@]}"}"; do [[ "$s" == "$step" ]] && return 1; done
  for s in "${selected[@]}"; do [[ "$s" == "$step" ]] && return 0; done
  return 1
}

# Tools in a home directory, for this run and the steps after it.
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

# --- privileges -------------------------------------------------------------------------------

if [[ $(id -u) -eq 0 ]]; then
  as_root() { "$@"; }
elif have sudo && sudo -n true 2>/dev/null; then
  as_root() { sudo -n "$@"; }
else
  as_root() {
    echo "[dev-env] needs root for: $* (run as root, or give this user passwordless sudo)" >&2
    return 1
  }
fi

apt_ready=0
apt_install() {
  local missing=() pkg
  for pkg in "$@"; do
    dpkg-query -W -f='${Status}' "$pkg" 2>/dev/null | grep -q "install ok installed" || missing+=("$pkg")
  done
  [[ ${#missing[@]} -gt 0 ]] || return 0
  if [[ $apt_ready -eq 0 ]]; then
    as_root apt-get update -qq
    apt_ready=1
  fi
  note "apt: ${missing[*]}"
  as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends "${missing[@]}"
}

# The first package of each pair that apt knows, for names that differ between releases.
apt_available() {
  local pkg
  for pkg in "$@"; do
    if apt-cache show "$pkg" >/dev/null 2>&1; then
      printf '%s' "$pkg"
      return 0
    fi
  done
  return 1
}

java_major() {
  have java || { echo 0; return; }
  java -version 2>&1 | sed -n 's/.*version "\([0-9]*\).*/\1/p' | head -1
}

node_major() {
  have node || { echo 0; return; }
  node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0
}

# --- check ------------------------------------------------------------------------------------

report() {
  local tool version
  for tool in git curl gcc cmake pkg-config java node npm python3 uv just rustup cargo rustc \
    cargo-release git-graph google-chrome claude cursor-agent gh jq; do
    if have "$tool"; then
      case "$tool" in
        # The JVM prints proxy settings ("Picked up ...") ahead of its version on some machines.
        java) version="$(java -version 2>&1 | grep -v '^Picked up' | head -1)" ;;
        # Run on its own, cargo-release expects to be called as `cargo release`.
        cargo-release) version="$(cargo release --version 2>&1 | head -1)" ;;
        *) version="$("$tool" --version 2>&1 | head -1)" ;;
      esac || true
      printf '  %-14s %s\n' "$tool" "$version"
    else
      printf '  %-14s missing\n' "$tool"
    fi
  done
  if [[ -n "$repo_dir" ]]; then
    printf '  %-14s %s\n' "checkout" "$repo_dir"
  fi
}

if [[ $check_only -eq 1 ]]; then
  report
  exit 0
fi

# --- steps ------------------------------------------------------------------------------------

step_system() {
  if ! have apt-get; then
    note "no apt-get: install a C compiler, cmake, pkg-config, git, curl, jq, and Java 17+ yourself"
    return 0
  fi
  local packages=(
    ca-certificates curl git gnupg build-essential pkg-config cmake perl unzip xz-utils jq
    procps lsof netcat-openbsd python3 python3-venv fonts-liberation
  )
  # DynamoDB Local needs Java 17 or newer.
  if [[ "$(java_major)" -lt 17 ]]; then
    packages+=("$(apt_available openjdk-21-jre-headless default-jre-headless)")
  fi
  have gh || { gh_pkg="$(apt_available gh)" && packages+=("$gh_pkg"); } || true
  apt_install "${packages[@]}"
}

step_node() {
  local major
  major="$(node_major)"
  if [[ "$major" -ge $NODE_MIN ]] && have npm; then
    note "node $(node --version) is new enough"
    return 0
  fi
  have apt-get || { note "install Node.js $NODE_MAJOR yourself"; return 1; }
  note "installing Node.js $NODE_MAJOR from NodeSource (found ${major:-none})"
  apt_install ca-certificates curl gnupg
  curl -fsSL "https://deb.nodesource.com/setup_${NODE_MAJOR}.x" | as_root bash -
  as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq nodejs
}

step_rust() {
  if ! have rustup; then
    # rustup-init straight from static.rust-lang.org, the host that serves the toolchains too.
    local target init
    case "$(uname -m)" in
      x86_64 | amd64) target=x86_64-unknown-linux-gnu ;;
      aarch64 | arm64) target=aarch64-unknown-linux-gnu ;;
      *) echo "[dev-env] no rustup for $(uname -m)" >&2; return 1 ;;
    esac
    note "installing rustup (stable, minimal profile)"
    init="$(mktemp -d)/rustup-init"
    curl --proto '=https' --tlsv1.2 -fsSL -o "$init" \
      "https://static.rust-lang.org/rustup/dist/$target/rustup-init"
    chmod +x "$init"
    "$init" -y --no-modify-path --profile minimal --default-toolchain stable
    rm -f "$init"
  fi
  rustup toolchain install stable --profile minimal --component rustfmt,clippy --no-self-update
  rustup default stable >/dev/null
  note "$(rustc --version)"
}

step_tools() {
  if ! have uv; then
    note "installing uv"
    curl -LsSf https://astral.sh/uv/install.sh | env UV_NO_MODIFY_PATH=1 sh
  fi
  if ! have just; then
    # Prebuilt, the same package CI installs with pipx.
    note "installing just"
    uv tool install rust-just
  fi
  if [[ "${DQL_SKIP_CARGO_TOOLS:-0}" == 1 ]]; then
    note "skipping cargo-release and git-graph (DQL_SKIP_CARGO_TOOLS=1)"
    return 0
  fi
  have cargo || { note "cargo is missing: run the rust step first"; return 1; }
  local crate
  for crate in cargo-release git-graph; do
    if ! have "$crate"; then
      note "cargo install $crate"
      cargo install --locked --quiet "$crate"
    fi
  done
}

step_agents() {
  if have claude; then
    note "claude $(claude --version 2>/dev/null | head -1)"
  else
    note "installing Claude Code"
    curl -fsSL https://claude.ai/install.sh | bash
  fi
  if have cursor-agent; then
    note "cursor-agent $(cursor-agent --version 2>/dev/null | head -1)"
  else
    note "installing the Cursor CLI"
    curl -fsSL https://cursor.com/install | bash
  fi
}

step_browser() {
  # playwright.config.js uses channel "chrome": Playwright looks for /opt/google/chrome/chrome.
  if [[ -x /opt/google/chrome/chrome ]]; then
    note "$(/opt/google/chrome/chrome --version 2>/dev/null || echo "chrome at /opt/google/chrome")"
    return 0
  fi
  have apt-get || { note "install Google Chrome yourself"; return 1; }
  local arch deb
  arch="$(dpkg --print-architecture)"
  if [[ "$arch" == amd64 ]]; then
    note "installing Google Chrome"
    deb="$(mktemp -d)/chrome.deb"
    curl -fsSL -o "$deb" https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb
    if [[ $apt_ready -eq 0 ]]; then
      as_root apt-get update -qq
      apt_ready=1
    fi
    as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "$deb"
    rm -f "$deb"
    return 0
  fi
  # Google Chrome has no Linux build for this architecture. Playwright's Chromium stands in, at
  # the path the "chrome" channel and notebook-tests/run.sh look for.
  have npx || { note "npx is missing: run the node step first"; return 1; }
  note "no Google Chrome for $arch: installing Playwright's Chromium in its place"
  local browsers="/opt/pw-browsers"
  as_root mkdir -p "$browsers"
  as_root env PLAYWRIGHT_BROWSERS_PATH="$browsers" npx -y playwright@1.56.1 install --with-deps chromium
  local binary
  binary="$(find "$browsers" -path '*chrome-linux*/chrome' -type f | head -1)"
  [[ -n "$binary" ]] || { echo "[dev-env] Chromium did not install" >&2; return 1; }
  as_root chmod -R a+rX "$browsers"
  as_root mkdir -p /opt/google/chrome
  as_root ln -sf "$binary" /opt/google/chrome/chrome
  as_root ln -sf /opt/google/chrome/chrome /usr/local/bin/google-chrome
}

step_repo() {
  if [[ $no_repo -eq 1 ]]; then
    note "skipping the repo step (--no-repo)"
    return 0
  fi
  if [[ -z "$repo_dir" ]]; then
    note "no checkout found: set DQL_REPO_DIR or run the copy of this script in the repo"
    return 0
  fi
  cd "$repo_dir"
  note "checkout: $repo_dir"
  (cd py-impl && uv sync --dev --quiet)
  (cd notebook-tests && uv sync --quiet)
  (cd rust-impl && cargo fetch --locked --quiet)
  (cd dqlrs-web/playwright && [[ -x node_modules/.bin/playwright ]] ||
    PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 npm ci --no-audit --no-fund --silent)
  # The jar that `just dynamo` starts, downloaded now so the first test run does not wait for it.
  local dynamo=".dynamo-local"
  if [[ ! -f "$dynamo/DynamoDBLocal.jar" ]]; then
    note "downloading DynamoDB Local"
    mkdir -p "$dynamo/data"
    (cd "$dynamo" &&
      curl -fsSLO https://d1ni2b6xgvw0s0.cloudfront.net/v2.x/dynamodb_local_latest.tar.gz &&
      tar -xzf dynamodb_local_latest.tar.gz)
  fi
}

# Each step runs in its own subshell with errexit on, so its first failing command ends it.
# (Bash ignores set -e inside a function that is called from an if or a ||.)
failed=()
for step in "${STEPS_ALL[@]}"; do
  wanted "$step" || continue
  log "$step"
  set +e
  (set -e; "step_$step")
  status=$?
  set -e
  if [[ $status -ne 0 ]]; then
    failed+=("$step")
    echo "[dev-env] step $step failed" >&2
  fi
done

log "installed"
report
if [[ ${#failed[@]} -gt 0 ]]; then
  echo "[dev-env] failed steps: ${failed[*]}" >&2
  exit 1
fi
