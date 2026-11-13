# TODO: Prebuilt binaries and a working installer

**Priority:** High  
**Area:** Release packaging, CI, `scripts/rust-install.sh`  
**Status:** Not started. The README offers only git-URL installs until this is done.

## Problem

There is no way to install `dqlrs` without a Rust toolchain. The README used to
advertise a one-line installer, and it fails every way it can be run (checked
2026-11-13):

| Step | What happens |
| --- | --- |
| `curl … rust-install.sh \| sh` | The script is bash (`set -o pipefail`, `[[ ]]`, `local`). On Debian and Ubuntu `sh` is dash, so it stops on line 3: `set: Illegal option -o pipefail`. |
| `curl … rust-install.sh \| bash` | `api.github.com/repos/jspreddy/dql/releases/latest` returns 404. The repository has no GitHub releases. Its only tags are old Python ones (`0.5.x`). |
| After a release exists | The script would still fail, because it does not match what `rust-release.yml` publishes (table below). |

The script and the release workflow disagree:

| | `.github/workflows/rust-release.yml` publishes | `scripts/rust-install.sh` expects |
| --- | --- | --- |
| Asset name | `dqlrs-<tag>-<linux\|macos>-<uname -m>.tar.gz` | `dql-.*-<linux\|macos>-<x86_64\|aarch64>.tar.gz` |
| Binary in the archive | `dqlrs` | `dql`, installed as `dql` |
| Apple Silicon arch | `arm64` (`uname -m` on the macOS runner) | `aarch64` |
| Linux ARM | not built | `aarch64` |

Other loose ends:

- `bin/install-rust.sh` is a byte-for-byte copy of `scripts/rust-install.sh`.
- `just rust-install-release` runs the broken script. Its comment still says it
  installs "the release dql binary".
- GitHub Actions is not running green, so `rust-release.yml` (triggered by
  `rust-X.Y.Z` tags) has never produced a release.
- `rust-docs/packaging.html` and `justfile.html` draw the release-script path
  as if it worked.

## What works today

These are the only install paths in the README. Both were run end to end on
2026-11-13:

```bash
# Rust dqlrs: builds from source, needs Rust/Cargo
cargo install --git https://github.com/jspreddy/dql.git --branch v-rust \
  --locked --root ~/.local dql-cli

# Python dql: needs uv; uv fetches Python 3.9
uv tool install --python 3.9 \
  "git+https://github.com/jspreddy/dql.git@v-rust#subdirectory=py-impl"
```

## Recommendation

1. **Get CI green first.** Fix `rust-workflows.yml` so pushes to `v-rust` and
   pull requests pass. A release job that never runs cannot be trusted.
2. **Pick one asset contract and make both sides use it.** Suggested:
   `dqlrs-<version>-<os>-<arch>.tar.gz`, where `<os>` is `linux` or `macos` and
   `<arch>` is normalized to `x86_64` or `aarch64` in the workflow (map
   `arm64` → `aarch64`). The archive holds `dqlrs` and `LICENSE`.
3. **Rewrite the installer to match.**
   - POSIX `sh`, so `curl … | sh` works on dash, or change every doc to
     `| bash`. POSIX `sh` is the safer choice.
   - Look for `dqlrs-*` assets and install `dqlrs`, not `dql`.
   - Do not need Python to parse the API response (`python3` is not on every
     machine). Prefer the `releases/latest/download/<asset>` redirect with a
     fixed asset name, or plain `grep`/`sed`.
   - Verify a published SHA-256 checksum before installing.
   - Say clearly when there is no release for this platform, instead of
     printing a Python traceback.
4. **Build the platforms the installer claims.** Add Linux `aarch64`, or drop
   it from the installer. Consider a static `x86_64-unknown-linux-musl` build.
5. **Delete the duplicate** `bin/install-rust.sh`, and fix the
   `rust-install-release` recipe comment.
6. **Cut a first release** from a `rust-X.Y.Z` tag. Check that the release page
   has every asset, then run the installer on Linux x86_64, macOS arm64, and
   macOS x86_64 if it is still built.
7. **Then restore the one-line installer** in `README.md` (quick start and
   Install), `rust-docs/README.md`, and `rust-docs/migration-from-python.md`,
   and redraw `rust-docs/packaging.html`.

Optional: a Homebrew tap, and `cargo binstall` metadata pointing at the same
assets.

## Done when

- A `rust-X.Y.Z` tag produces a GitHub release with Linux and macOS archives
  and checksums, built by CI.
- `curl -fsSL …/scripts/rust-install.sh | sh` installs a working `dqlrs` on a
  clean Ubuntu (dash) machine and on macOS, without Python or Rust installed.
- On a platform with no asset, the installer prints a one-line error naming the
  platform and exits non-zero.
- The README quick start uses the installer again, and the git-URL installs
  stay documented as the from-source option.
