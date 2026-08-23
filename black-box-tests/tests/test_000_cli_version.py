from cli import Cli


def test_000_cli_version(cli: Cli) -> None:
    cli.assert_stdout("version", "0.6.4")
