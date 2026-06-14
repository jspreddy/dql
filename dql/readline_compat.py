"""Configure readline for cross-platform REPL support.

cmd.Cmd imports the stdlib ``readline`` module at runtime.  We prefer
``gnureadline`` for consistent behavior across Python builds, so we register
it as ``readline`` in ``sys.modules`` before anything else imports readline.
"""

import sys
from typing import Any, Optional

readline: Optional[Any] = None

try:
    import gnureadline

    sys.modules["readline"] = gnureadline
    readline = gnureadline
except ImportError:
    try:
        import readline as _stdlib_readline

        readline = _stdlib_readline
    except ImportError:
        pass

if readline is not None:
    try:
        import rlcompleter  # noqa: F401  # registers the Completer class

        # Mac OS X readline compatibility from http://stackoverflow.com/a/7116997
        if "libedit" in str(readline.__doc__):
            readline.parse_and_bind("bind ^I rl_complete")
        else:
            readline.parse_and_bind("tab: complete")

        # Tab-complete names with a '-' in them
        delims = set(readline.get_completer_delims())
        if "-" in delims:
            delims.remove("-")
            readline.set_completer_delims("".join(delims))
    except Exception:
        # Non-interactive terminals may not support readline configuration.
        pass
