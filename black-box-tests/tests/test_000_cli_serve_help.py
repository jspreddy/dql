from cli import Cli


def test_000_cli_serve_help(cli: Cli) -> None:
    """dqlrs --help lists --serve, --bind, and notebook; --serve -c is rejected."""
    cli.require_dqlrs()
    help_out = cli.run_args(["--help"])
    for needle in ("--serve", "--bind", "notebook"):
        if needle not in help_out.stdout:
            raise AssertionError("help missing %r:\n%s" % (needle, help_out.stdout))
    rejected = cli.run_args(["--serve", "-c", "opt"], check=False)
    if rejected.exitstatus == 0:
        raise AssertionError("--serve -c should fail\n%s" % rejected.stdout)
