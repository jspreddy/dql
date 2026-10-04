import re
from pathlib import Path

from cli import Cli

_ROOT = Path(__file__).resolve().parents[2]


def _declared_version(label: str) -> str:
    if label == "dqlrs":
        path = _ROOT / "rust-impl" / "Cargo.toml"
    else:
        path = _ROOT / "py-impl" / "pyproject.toml"
    match = re.search(r'(?m)^version = "([^"]+)"', path.read_text())
    if match is None:
        raise AssertionError("no version in %s" % path)
    return match.group(1)


def test_000_cli_version(cli: Cli) -> None:
    """Print the client version from the REPL version meta-command."""
    cli.assert_stdout("version", _declared_version(cli.label))
