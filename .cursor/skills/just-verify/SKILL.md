---
name: just-verify
description: Run `just verify` and report only the recipe headings and the exit code. Use when the user asks to verify, run just verify, or invoke this skill. Do not read the verify log.
---

# Just verify

Run the suite so its log stays on disk. The command prints the `[## recipe ##]` headings and the exit code. That printed summary is the whole result.

From the repository root:

```bash
.cursor/skills/just-verify/run.sh
```

Give the command enough time to finish. A full run is several minutes. Do not stream `just verify` itself into the conversation. Do not `cat`, `tail`, or read the log while it runs or after it finishes.

Show the headings and the `exit:` line. Stop there.

The log path is the `log:` line. Leave that file unread. Open it only when the user asks what failed, and then only through the script:

```bash
.cursor/skills/just-verify/run.sh --failed
```

`--failed` prints the recipe heading, the assertion, and the `error: recipe` line. Do not ask it for more of the log, and do not open the log any other way.

To repeat the last summary without running the suite again:

```bash
.cursor/skills/just-verify/run.sh --from-log
```
