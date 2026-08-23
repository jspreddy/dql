from cli import Cli


def test_310_journeys_getting_started_posts(cli: Cli) -> None:
    table = cli.table()
    cli.oneshot(
        f"CREATE TABLE {table} (username STRING HASH KEY, postid NUMBER RANGE KEY,"
        f" ts NUMBER INDEX('ts-index'), THROUGHPUT (5, 5));\n"
        f"INSERT INTO {table} (username, postid, ts, text) VALUES"
        f" ('steve', 1, 1386413481, 'Hey guys!'),"
        f" ('steve', 2, 1386413516, 'Guys?'),"
        f" ('drdice', 1, 1386413575, 'No one here');"
    )
    cli.assert_json(
        f"SELECT * FROM {table} WHERE username = 'steve';",
        [
            {"username": "steve", "postid": 1, "ts": 1386413481, "text": "Hey guys!"},
            {"username": "steve", "postid": 2, "ts": 1386413516, "text": "Guys?"},
        ],
    )
