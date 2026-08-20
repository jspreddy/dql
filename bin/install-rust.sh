#!/usr/bin/env bash
# Install the Rust `dql` binary from GitHub releases.
set -euo pipefail

# Default to this fork. Override with DQL_REPO=stevearc/dql for upstream.
REPO="${DQL_REPO:-jspreddy/dql}"
VERSION="${DQL_VERSION:-latest}"
INSTALL_DIR="${DQL_INSTALL_DIR:-}"

detect_platform() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"
  case "$os" in
    linux) os="linux" ;;
    darwin) os="macos" ;;
    *)
      echo "error: unsupported OS: $os" >&2
      exit 1
      ;;
  esac
  case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *)
      echo "error: unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac
  printf '%s-%s' "$os" "$arch"
}

choose_install_dir() {
  if [[ -n "$INSTALL_DIR" ]]; then
    printf '%s' "$INSTALL_DIR"
    return
  fi
  if [[ -w /usr/local/bin ]] || mkdir -p /usr/local/bin 2>/dev/null; then
    printf '%s' "/usr/local/bin"
    return
  fi
  printf '%s' "${HOME}/.local/bin"
}

fetch_release_url() {
  local platform="$1"
  local api asset_pattern
  if [[ "$VERSION" == "latest" ]]; then
    api="https://api.github.com/repos/${REPO}/releases/latest"
  else
    api="https://api.github.com/repos/${REPO}/releases/tags/${VERSION}"
  fi
  asset_pattern="dql-.*-${platform}\\.tar\\.gz"
  curl -fsSL "$api" | python3 - "$asset_pattern" <<'PY'
import json, re, sys
pattern = re.compile(sys.argv[1])
data = json.load(sys.stdin)
for asset in data.get("assets", []):
    name = asset.get("name", "")
    if pattern.search(name):
        print(asset["browser_download_url"])
        break
else:
    raise SystemExit("asset not found for this platform")
PY
}

main() {
  local platform install_dir url tmp archive
  platform="$(detect_platform)"
  install_dir="$(choose_install_dir)"
  mkdir -p "$install_dir"

  echo "Installing dql for ${platform} into ${install_dir}"
  url="$(fetch_release_url "$platform")"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  archive="${tmp}/dql.tar.gz"
  curl -fsSL "$url" -o "$archive"
  tar -xzf "$archive" -C "$tmp"
  install -m 0755 "${tmp}/dql" "${install_dir}/dql"

  if ! command -v dql >/dev/null 2>&1; then
    echo "Installed ${install_dir}/dql"
    echo "Add ${install_dir} to your PATH if needed."
  fi
  "${install_dir}/dql" --version
}

main "$@"
