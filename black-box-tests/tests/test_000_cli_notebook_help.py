from cli import Cli


def test_000_cli_notebook_help(cli: Cli) -> None:
    """dqlrs notebook --help describes JupyterLab; --notebook --serve is rejected."""
    cli.require_dqlrs()
    help_out = cli.run_args(["notebook", "--help"])
    text = help_out.stdout
    if "JupyterLab" not in text or "DQL" not in text:
        raise AssertionError("notebook help should describe JupyterLab:\n%s" % text)
    flag_help = cli.run_args(["--notebook", "--help"])
    if "JupyterLab" not in flag_help.stdout:
        raise AssertionError("--notebook --help should describe JupyterLab")
    rejected = cli.run_args(["--notebook", "--serve"], check=False)
    if rejected.exitstatus == 0:
        raise AssertionError("--notebook --serve should fail\n%s" % rejected.stdout)
