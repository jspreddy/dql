"""Launch the DQL wrapper kernel: python -m dql_notebook_kernel."""

from __future__ import annotations

import argparse
import os
import sys

from ipykernel.kernelapp import IPKernelApp

from .kernel import DqlKernel


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(prog="dql_notebook_kernel")
    parser.add_argument(
        "--backend",
        choices=("python", "rust"),
        default=None,
        help="Which DQL binary to run (python=dql, rust=dqlrs)",
    )
    args, remaining = parser.parse_known_args(argv)
    if args.backend:
        os.environ["DQL_NOTEBOOK_BACKEND"] = args.backend
    sys.argv = [sys.argv[0], *remaining]
    IPKernelApp.launch_instance(kernel_class=DqlKernel)


if __name__ == "__main__":
    main()
