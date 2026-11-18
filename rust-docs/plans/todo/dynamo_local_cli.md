# `dqlrs dynamo`: list, start, stop, and clean DynamoDB Local

Status: done on `handle-ddb-local` (Docker paths are unit-tested; the Java path was run end to end).

## Why

`dqlrs web --init-examples` starts DynamoDB Local with Java or Docker, prints how to stop it,
and forgets it. Nothing finds that server again, the Java download lands in every workspace,
and the web UI cannot tell whether the local endpoint is up.

## Decisions

| Question | Decision |
| --- | --- |
| Command name | `dqlrs dynamo` (matches `just dynamo` / `dd`; `local` is taken by the REPL command and `notebook --local`) |
| Installs and data | One shared Java download in `~/.local/share/dqlrs/dynamodb-local/install` (`DQLRS_DYNAMO_HOME` overrides the root). Data per workspace (`<dir>/.dynamodb-local/data`), `--data` to choose another folder |
| Several servers | Yes, one per port. `start` uses 8000; `--port N` or `--port auto` (next free port) |
| Docker | Keep compose. `--init-examples` and a workspace that has the examples' `docker-compose.yml` use it; elsewhere `start` writes a compose file under the shared folder |
| Destructive commands | `clean` asks before deleting data; `--yes` skips. `--purge` also removes the Java download or the Docker image. Servers dqlrs did not start are only touched when named, never by `--all`, and their data is never deleted |
| Scope | The CLI, `--init-examples` on top of it, and the server's status in the web UI's endpoint chip. `just dd-*` stays as it is |

## Commands

```
dqlrs dynamo list [--json]
dqlrs dynamo start [--java | --docker] [--port N|auto] [--dir DIR] [--data DIR]
dqlrs dynamo stop  [PORT | --all]
dqlrs dynamo clean [PORT | --all] [--purge] [--yes]
dqlrs dynamo logs  [PORT]
```

A PORT can be left out when exactly one server is known.

## How servers are found

- **Records.** `start` writes `<home>/instances/<port>.json`: runtime, port, pid or compose
  file, data, workspace, start time. Java logs go to `<home>/instances/<port>.log`.
- **Scanning.** `java … -jar DynamoDBLocal.jar` processes (port from `-port`, else 8000) and
  `amazon/dynamodb-local` containers (`docker ps -a`), so servers started elsewhere show as
  `other`.
- **Status.** Running when the process or container is up and the port answers like DynamoDB.
  A record whose server is gone shows as stopped; `start` on that port restarts it with the
  same settings.

## Web UI

`GET /api/dynamo` reports the server behind the current endpoint when it is local: runtime,
status, data, started. The endpoint chip shows a status dot (running or not answering), and
its details popover adds the server's runtime, data, and the `dqlrs dynamo` commands.

## Out of scope

- `just dd-*` recipes (they keep their scripts).
- Starting or stopping from the web UI.
- Windows process scanning (Java servers started elsewhere are not listed there; records still are).

## Steps

1. `dynamo` module: home, records, scanning, start, stop, clean, logs; unit tests.
2. `dqlrs dynamo …` argument parsing and help.
3. `--init-examples` starts through the module; its messages point at `dqlrs dynamo`.
4. `/api/dynamo` and the endpoint chip; a Playwright test with screenshots.
5. Docs: `rust-docs/README.md` and the top-level README.
