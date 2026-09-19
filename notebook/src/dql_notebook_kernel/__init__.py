"""Jupyter wrapper kernels and IPython magics for DQL."""

from .magics import DqlMagics, load_ipython_extension

__all__ = ["DqlMagics", "load_ipython_extension"]
__version__ = "0.1.0"
