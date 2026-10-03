"""JSON/text compare helpers for black-box CLI output. No dql import."""

from __future__ import annotations

import json
from decimal import Decimal, InvalidOperation
from typing import Any


def parse_cli_json(stdout: str) -> Any:
    """Parse --json CLI output: one JSON value, an array, or concatenated objects."""
    text = stdout.strip()
    if not text:
        return []
    try:
        parsed = json.loads(text)
        if isinstance(parsed, dict):
            return [parsed]
        return parsed
    except json.JSONDecodeError:
        pass
    decoder = json.JSONDecoder()
    idx = 0
    values: list[Any] = []
    length = len(text)
    while idx < length:
        while idx < length and text[idx].isspace():
            idx += 1
        if idx >= length:
            break
        try:
            value, end = decoder.raw_decode(text, idx)
        except json.JSONDecodeError:
            nxt_obj = text.find("{", idx + 1)
            nxt_arr = text.find("[", idx + 1)
            candidates = [n for n in (nxt_obj, nxt_arr) if n != -1]
            if not candidates:
                leftover = text[idx:].strip()
                if leftover and not values:
                    raise ValueError(
                        "could not parse JSON from CLI stdout:\n" + leftover[:500]
                    ) from None
                break
            idx = min(candidates)
            continue
        values.append(value)
        idx = end
    if not values:
        raise ValueError("no JSON values found in CLI stdout:\n" + text[:500])
    if all(isinstance(value, dict) for value in values):
        return values
    if len(values) == 1:
        return values[0]
    return values


def _is_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def json_equal(left: Any, right: Any) -> bool:
    """Structural equality: object keys unordered, numbers 1 == 1.0, item lists sorted."""
    if _is_number(left) and _is_number(right):
        try:
            return Decimal(str(left)) == Decimal(str(right))
        except InvalidOperation:
            return False
    if isinstance(left, dict) and isinstance(right, dict):
        if set(left) != set(right):
            return False
        return all(json_equal(left[key], right[key]) for key in left)
    if isinstance(left, list) and isinstance(right, list):
        if len(left) != len(right):
            return False
        if left and all(isinstance(item, dict) for item in left + right):
            return _sorted_dicts(left) == _sorted_dicts(right)
        return all(json_equal(a, b) for a, b in zip(left, right))
    return left == right


def _sorted_dicts(items: list[Any]) -> list[str]:
    canonical = []
    for item in items:
        canonical.append(json.dumps(_normalize(item), sort_keys=True, separators=(",", ":")))
    canonical.sort()
    return canonical


def _normalize(value: Any) -> Any:
    if _is_number(value):
        return format(Decimal(str(value)).normalize(), "f")
    if isinstance(value, dict):
        return {key: _normalize(val) for key, val in value.items()}
    if isinstance(value, list):
        return [_normalize(item) for item in value]
    return value


def stdout_matches(actual: str, expected: str) -> bool:
    actual_n = actual.replace("\r\n", "\n")
    expected_n = expected.replace("\r\n", "\n")
    if actual_n == expected_n:
        return True
    if actual_n.rstrip("\n") == expected_n.rstrip("\n"):
        return True
    want = expected_n.strip()
    if want and want in actual_n:
        return True
    actual_ws = " ".join(actual_n.split())
    expected_ws = " ".join(expected_n.split())
    return expected_ws != "" and expected_ws in actual_ws


def self_test() -> list[str]:
    """Return names of failing checks (empty means pass)."""
    failures: list[str] = []

    def check(name: str, cond: bool) -> None:
        if not cond:
            failures.append(name)

    check(
        "array json",
        json_equal(parse_cli_json('[{"id": "a", "n": 1}]'), [{"id": "a", "n": 1}]),
    )
    concatenated = '{\n    "id": "a",\n    "n": 1\n}\n{\n    "id": "b",\n    "n": 2\n}\n'
    check(
        "concat objects",
        json_equal(parse_cli_json(concatenated), [{"id": "a", "n": 1}, {"id": "b", "n": 2}]),
    )
    check("single object is item list", json_equal(parse_cli_json('{"id": "a"}'), [{"id": "a"}]))
    check(
        "trailing non-json after object",
        json_equal(
            parse_cli_json('{\n    "id": "a"\n}\n\nquery\n  Table: R:0.5\n'),
            [{"id": "a"}],
        ),
    )
    check("number 1 vs 1.0", json_equal([{"n": 1}], [{"n": 1.0}]))
    check(
        "unordered items",
        json_equal([{"id": "b"}, {"id": "a"}], [{"id": "a"}, {"id": "b"}]),
    )
    check("stdout substring", stdout_matches("hello world\n", "world"))
    check("stdout exact", stdout_matches("hello\n", "hello\n"))
    check(
        "stdout wrapped substring",
        stdout_matches("THROUGHPUT (2, \n3));\n", "(2, 3)"),
    )
    return failures
