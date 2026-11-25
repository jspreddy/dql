# Dynamo stop kills the wrong process on Unix

Status: fixed in #313 (`792584f`), test in #310. Follow-up: [`dynamo-clean-refuses-a-stale-record.md`](dynamo-clean-refuses-a-stale-record.md).

## What goes wrong

On Unix, `dqlrs dynamo stop` and `dqlrs dynamo clean` send `kill` to the pid stored when the Java server started. They do not check that the process is still `java -jar DynamoDBLocal.jar`.

A recorded pid stays “running” when `kill -0` succeeds and the port is open. Those checks are not the same process. The pid can belong to a new process while something else is listening on the port. Stop then kills that pid.

Scanning for servers already ignores a command line that is not the DynamoDB Local jar. Stop does not.

## Why

`pid_alive` in `rust-impl/crates/dql-cli/src/dynamo.rs` is `kill -0`. That only means some process has the number. `servers` marks a Java record running when `pid_alive` and `port_open` are both true. `stop` then runs `kill <pid>` with no further check. Nothing ties the pid to the port or to `DynamoDBLocal.jar`.

`parse_java_processes` already requires `-jar` and `DynamoDBLocal.jar`. The scan uses that. The stop path does not.

## What to change

Before killing a Java pid on Unix, read its command line with `ps -ww -p <pid> -o pid=,command=` so the line is not the truncated `ps -ax` listing. If that listing succeeds and the pid is not a DynamoDB Local server under the same rule as `parse_java_processes`, refuse to kill it and say the recorded process is gone. If `ps` itself fails, keep today's behavior so a missing `ps` does not make stop impossible.

Add a unit test on a fake process listing: the jar pid may be killed, and an unrelated command with the same pid may not.
