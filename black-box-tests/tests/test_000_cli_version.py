from cli import Cli


def test_000_cli_version(cli: Cli) -> None:
    """Print the client version from the REPL version meta-command."""
    cli.assert_stdout("version", "0.6.4")
