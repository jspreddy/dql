from cli import Cli


def test_000_cli_serve_ping(cli: Cli) -> None:
    """dqlrs --serve ping returns ok over stdio JSON-lines."""
    cli.require_dqlrs()
    with cli.serve() as session:
        reply = session.request("ping")
    if reply.get("ok") is not True:
        raise AssertionError("ping envelope not ok: %s" % reply)
