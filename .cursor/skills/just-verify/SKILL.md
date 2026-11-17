---
name: just-verify
description: Run `just verify` and show its printed summary unchanged. Use when the user asks to verify, run just verify, or invoke this skill. Do not read the verify log. Do not rewrite the summary.
---

# Just verify

Run the suite so its log stays on disk. The command greps that log and prints the `[## recipe ##]` headings, the `exit:` line, the `log:` line, and the result line. That printed text is the whole result.

From the repository root:

```bash
.cursor/skills/just-verify/run.sh
```

Give the command enough time to finish. A full run is several minutes. Do not stream `just verify` itself into the conversation. Do not `cat`, `tail`, or read the log while it runs or after it finishes.

The reply is the command's printed output, copied exactly. Do not rewrite it. Do not turn the headings into a list of your own. Do not replace the result line with a sentence of your own. Do not add a line before or after the printed output.

The log path is the `log:` line inside that output. Leave that file unread. Do not open the log.

To repeat the last summary without running the suite again, run this and copy its printed output the same way:

```bash
.cursor/skills/just-verify/run.sh --from-log
```
