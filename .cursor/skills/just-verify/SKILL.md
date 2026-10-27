---
name: just-verify
description: Run `just verify` and report the recipe headings, the exit code, and whether the suite was successful. Use when the user asks to verify, run just verify, or invoke this skill. Do not read the verify log.
---

# Just verify

Run the suite so its log stays on disk. The command prints the `[## recipe ##]` headings, the exit code, and whether the suite was successful. That printed summary is the whole result.

From the repository root:

```bash
.cursor/skills/just-verify/run.sh
```

Give the command enough time to finish. A full run is several minutes. Do not stream `just verify` itself into the conversation. Do not `cat`, `tail`, or read the log while it runs or after it finishes.

Show the headings and the `exit:` line. Then report the result line that follows them:

- `The verify suite was successful.` means the run succeeded. Tell the user it was successful.
- `The verify suite failed.` means a recipe failed. The lines under it name the recipe and the error. Describe what failed from those lines.

The log path is the `log:` line. Leave that file unread. Do not open the log.

To repeat the last summary without running the suite again:

```bash
.cursor/skills/just-verify/run.sh --from-log
```
