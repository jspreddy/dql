# Release plans: prebuilt binaries and `dqlrs update`

Status: planned. Nothing here is built yet.

Today a release is a `dqlrs-<version>` tag made by the **Release dqlrs** workflow
([`release.yml`](../.github/workflows/release.yml)). Users build from source with
`cargo install --tag dqlrs-<version>` or `--branch v-rust`. This plan covers the next
two steps: binaries that install without a Rust toolchain, and a built-in update command.

## Open questions

These are undecided. Each section below notes where the answer changes the work.

1. **GitHub Release pages and binaries.** Should a tag get a GitHub Release page, and
   should it carry prebuilt binaries now or later? A Release page alone is cheap (notes
   from the changelog). Binaries add a build matrix to every release.
2. **How `dqlrs update` installs.** Should it start by running `cargo install` for the
   user (needs Rust), or replace itself with a downloaded binary from day one (needs
   section 1's binaries)?
3. **Version labels on `v-rust` builds.** A `v-rust` build between releases reports the
   last released version. Should `dqlrs --version` add the commit (`0.7.11+g1ee78f4`),
   or should `v-rust` move to the next pre-release after each release? Not needed while
   everything builds from source, but `dqlrs update` must tell builds apart.

## 1. Prebuilt binaries

### Builds

Extend `release.yml` with a job that runs after the tag is pushed (in the same run:
pushes made with the job token do not start other workflows).

| Target | Runner |
| --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` |
| `x86_64-apple-darwin` | `macos-13` |
| `aarch64-apple-darwin` | `macos-latest` |

Each job runs `cargo build --release --locked -p dql-cli` at the tag and packs
`dqlrs` with `LICENSE` into `dqlrs-<version>-<os>-<arch>.tar.gz`, with a
`.sha256` file next to it. Windows waits until someone asks.

Open points:

- Linux `gnu` builds depend on the runner's glibc. A `musl` static build avoids that
  but needs checking against the AWS SDK's TLS stack (`default-https-client`).
- macOS binaries are unsigned. Gatekeeper quarantines downloads from a browser, not
  from `curl`, so the install script works; signing waits.

### Publishing

The Actions policy allows only GitHub-made actions, so publish with the `gh` CLI:
`gh release create dqlrs-<version> --notes-file <section> dist/*`. The notes are that
version's section of `CHANGELOG.md`. Pre-releases get `--prerelease`.

Question 1 decides whether this ships with binaries or as notes only.

### Install script

[`bin/install-rust.sh`](../bin/install-rust.sh) already downloads release assets. Bring
it up to date:

- Asset names as above, and verify the `.sha256`.
- `DQL_VERSION=latest` (default) reads the latest release from the GitHub API.
  `DQL_VERSION=0.7.11` pins one. `DQL_VERSION=v-rust` runs
  `cargo install --branch v-rust`, since there is no binary for the branch.
- Install to `~/.local/bin` by default, as the README does today.

README: add `curl -LsSf https://raw.githubusercontent.com/jspreddy/dql/v-rust/bin/install-rust.sh | sh`
above the `cargo install` commands, and keep both cargo commands for people who build
from source. `scripts/release_pins.py` keeps pinning the cargo `--tag`.

### Checks

- A dry run of `release.yml` builds all targets without publishing.
- The install script gets a test that installs a published release into a temporary
  directory and runs `dqlrs --version`.

## 2. `dqlrs update`

### Command

```text
dqlrs update                    # latest stable release
dqlrs update --version 0.7.11   # a specific release
dqlrs update --channel v-rust   # the newest v-rust (builds from source)
dqlrs update --check            # say whether a newer release exists; change nothing
```

It prints the current and target versions and asks before changing anything. `--yes`
skips the question, like `dqlrs web --init-examples --yes`. Output uses the progress
and style modules (`web/progress.rs`, `style.rs`), so it matches `--init-examples`.

### Finding the latest release

`GET https://api.github.com/repos/jspreddy/dql/releases/latest`, or, with no Release
pages (question 1), `git ls-remote --tags` filtered to `dqlrs-*` and sorted by semver.
The `--channel v-rust` path needs no lookup.

`dqlrs` only contacts GitHub when the user runs `dqlrs update`. No background checks.

### Installing: two options (question 2)

**A. Run cargo.** `dqlrs update` runs the same `cargo install --git … --tag dqlrs-<v>
--locked --root <root> dql-cli` the README shows, with `<root>` taken from where the
running binary lives (`~/.local/bin/dqlrs` gives `~/.local`). It needs cargo on `PATH`
and takes minutes. Without cargo it prints the install link and stops. Smallest to
build, no new dependencies.

**B. Replace the binary.** Download the asset for this platform, check its `.sha256`,
unpack it next to the running binary, and swap it in (the `self-replace` crate handles
Windows and running executables). Needs section 1's binaries. `--channel v-rust` still
falls back to A.

Both: refuse to update a binary under a package manager's directory (such as
Homebrew's), and say how to update it there instead.

### Version comparison

Compare semver, pre-releases included (`0.8.0-rc.1 < 0.8.0`). Question 3 decides how a
`v-rust` build reports itself. Without a commit in the version, `--channel v-rust`
always reinstalls.

### Checks

- Unit tests for semver comparison, the tag filter, and the install root.
- A test that runs `dqlrs update --check` against a stubbed GitHub response.
- For B, a test that swaps a dummy binary in a temporary directory.

## Order

1. GitHub Release pages with changelog notes (if question 1 says yes).
2. Binaries and the install script.
3. `dqlrs update`, option A or B depending on question 2.
