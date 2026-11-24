# dqlrs dynamo stop kills the recorded pid without checking the process

Status: open. Found while reviewing `dqlrs dynamo` in `0.7.12-alpha.1`.

## What goes wrong

`dqlrs dynamo stop` and `dqlrs dynamo clean` send `kill` (or `taskkill` on Windows) to the pid stored when the Java server started. They do not check that the process is still `java -jar DynamoDBLocal.jar`.

A Java server counts as running when the recorded pid is alive and the port is open. Those two checks are separate. The pid can belong to a new process while something else is listening on the port. Stop then kills that pid.

Scanning for servers already ignores a process whose command line is not the DynamoDB Local jar (`parse_java_processes` in `rust-impl/crates/dql-cli/src/dynamo.rs`). Stop does not use that check.

## Why

`stop` in `dynamo.rs` takes `server.pid` and runs `kill <pid>`. `pid_alive` is `kill -0`. `servers` sets a Java record to running only when `pid_alive` and `port_open` are both true. Nothing ties the pid to the port or to `DynamoDBLocal.jar`.

## What to change

- Before killing a Java pid, read its command line. On Unix, `ps -ww -p <pid> -o pid=,command=` avoids the truncated listing `ps -ax` can return.
- If the listing succeeds and the pid is not a DynamoDB Local server (`-jar` and `DynamoDBLocal.jar`, the same rule as `parse_java_processes`), refuse to kill it and say the recorded process is gone.
- If the listing command itself fails, keep today's behavior so a missing `ps` does not make stop impossible.
- Add a unit test on a fake process listing: the jar pid is allowed, an unrelated command with the same pid is not.
