"""Write project-local Jupyter kernelspecs for DQL and Python (dql)."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


KERNELS = (
    {
        "name": "dql-python",
        "display_name": "DQL (Python)",
        "backend": "python",
    },
    {
        "name": "dql-rust",
        "display_name": "DQL (Rust)",
        "backend": "rust",
    },
)


def write_dql_kernels(kernels_dir: Path, python_exe: str) -> list[Path]:
    written: list[Path] = []
    kernels_dir.mkdir(parents=True, exist_ok=True)
    for spec in KERNELS:
        dest = kernels_dir / spec["name"]
        dest.mkdir(parents=True, exist_ok=True)
        payload = {
            "argv": [
                python_exe,
                "-m",
                "dql_notebook_kernel",
                "-f",
                "{connection_file}",
                "--backend",
                spec["backend"],
            ],
            "display_name": spec["display_name"],
            "language": "dql",
            "interrupt_mode": "message",
            "metadata": {"debugger": False},
        }
        path = dest / "kernel.json"
        path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
        written.append(path)
    return written


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--prefix",
        required=True,
        help="Jupyter data prefix that contains a kernels/ directory",
    )
    parser.add_argument(
        "--python",
        default=sys.executable,
        help="Interpreter used in kernel argv (default: this Python)",
    )
    args = parser.parse_args(argv)
    prefix = Path(args.prefix)
    written = write_dql_kernels(prefix / "kernels", args.python)
    for path in written:
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
