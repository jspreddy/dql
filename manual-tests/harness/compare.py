#!/usr/bin/env python3
"""Compare CLI stdout/stderr/exit to expected files. Stdlib only; no dql import."""

from __future__ import annotations

import argparse
import json
import sys
from decimal import Decimal, InvalidOperation
from typing import Any


def parse_cli_json(stdout: str) -> Any:
    """Parse --json CLI output: one JSON value, an array, or concatenated objects."""
    text = stdout.strip()
    if not text:
        return []
    try:
        return json.loads(text)
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
                if leftover:
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
    if len(values) == 1:
        return values[0]
    if all(isinstance(value, dict) for value in values):
        return values
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
    return expected_n.strip() != "" and expected_n.strip() in actual_n


def stderr_matches(actual: str, expected: str) -> bool:
    if expected.strip() == "":
        return actual.strip() == ""
    return expected.strip() in actual


def load_text(path: str | None) -> str | None:
    if not path:
        return None
    with open(path, encoding="utf-8") as handle:
        return handle.read()


def compare(
    stdout: str,
    stderr: str,
    exit_code: int,
    expected_json: str | None,
    expected_stdout: str | None,
    expected_stderr: str | None,
    expected_exit: int,
) -> list[str]:
    errors: list[str] = []
    if exit_code != expected_exit:
        errors.append(f"exit code: got {exit_code}, expected {expected_exit}")
    if expected_json is not None:
        try:
            actual = parse_cli_json(stdout)
            want = json.loads(expected_json)
        except (ValueError, json.JSONDecodeError) as err:
            errors.append(f"json parse: {err}")
        else:
            if not json_equal(actual, want):
                errors.append(
                    "json mismatch\n--- actual ---\n"
                    + json.dumps(actual, indent=2, sort_keys=True, default=str)
                    + "\n--- expected ---\n"
                    + json.dumps(want, indent=2, sort_keys=True)
                )
    if expected_stdout is not None:
        if not stdout_matches(stdout, expected_stdout):
            errors.append(
                "stdout mismatch\n--- actual ---\n"
                + stdout
                + "\n--- expected ---\n"
                + expected_stdout
            )
    if expected_stderr is not None:
        if not stderr_matches(stderr, expected_stderr):
            errors.append(
                "stderr mismatch\n--- actual ---\n"
                + stderr
                + "\n--- expected ---\n"
                + expected_stderr
            )
    return errors


def _self_test() -> int:
    failures = 0

    def check(name: str, cond: bool) -> None:
        nonlocal failures
        if not cond:
            print(f"self-test FAIL: {name}", file=sys.stderr)
            failures += 1

    check(
        "array json",
        json_equal(parse_cli_json('[{"id": "a", "n": 1}]'), [{"id": "a", "n": 1}]),
    )
    concatenated = '{\n    "id": "a",\n    "n": 1\n}\n{\n    "id": "b",\n    "n": 2\n}\n'
    check(
        "concat objects",
        json_equal(parse_cli_json(concatenated), [{"id": "a", "n": 1}, {"id": "b", "n": 2}]),
    )
    check("empty stdout", parse_cli_json("") == [])
    check("number 1 vs 1.0", json_equal([{"n": 1}], [{"n": 1.0}]))
    check(
        "unordered items",
        json_equal([{"id": "b"}, {"id": "a"}], [{"id": "a"}, {"id": "b"}]),
    )
    check("stdout substring", stdout_matches("hello world\n", "world"))
    check("stdout exact", stdout_matches("hello\n", "hello\n"))
    check("stderr empty", stderr_matches("", ""))
    check("stderr nonempty fail", not stderr_matches("oops\n", ""))
    errs = compare("[]", "", 0, "[]", None, None, 0)
    check("compare pass", errs == [])
    errs = compare("[]", "", 0, '[{"id": "x"}]', None, None, 0)
    check("compare fail json", len(errs) == 1)
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--stdout", default="")
    parser.add_argument("--stderr", default="")
    parser.add_argument("--exit", type=int, default=0)
    parser.add_argument("--expected-json")
    parser.add_argument("--expected-stdout")
    parser.add_argument("--expected-stderr")
    parser.add_argument("--expected-exit", type=int, default=0)
    parser.add_argument("--stdout-file")
    parser.add_argument("--stderr-file")
    args = parser.parse_args()
    if args.self_test:
        return _self_test()

    stdout = load_text(args.stdout_file) if args.stdout_file else args.stdout
    stderr = load_text(args.stderr_file) if args.stderr_file else args.stderr
    expected_json = load_text(args.expected_json)
    expected_stdout = load_text(args.expected_stdout)
    expected_stderr = load_text(args.expected_stderr)
    if stdout is None or stderr is None:
        print("missing stdout/stderr", file=sys.stderr)
        return 2
    if expected_json is None and expected_stdout is None:
        print("need --expected-json and/or --expected-stdout", file=sys.stderr)
        return 2
    errors = compare(
        stdout,
        stderr,
        args.exit,
        expected_json,
        expected_stdout,
        expected_stderr,
        args.expected_exit,
    )
    if errors:
        for err in errors:
            print(err, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
