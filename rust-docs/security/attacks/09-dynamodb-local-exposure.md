# 09. DynamoDB Local: no auth, published on all interfaces, plaintext on disk

**Severity:** Medium. **Needs:** a TCP connection to the DynamoDB Local port (from this machine,
or from the network for Docker), or read access to its data folder.

## What happens

`dqlrs dynamo start`, `dqlrs web --local`, and the examples run DynamoDB Local for development.
It is a plain DynamoDB endpoint:

1. **No authentication.** DynamoDB Local accepts any access key and secret. Any process that
   reaches the port reads every table: `aws dynamodb scan --endpoint-url http://localhost:8000
   --table-name users`. No `dqlrs` token or grant is involved.
2. **Published on all interfaces with Docker.** The compose file that `dqlrs dynamo start
   --docker` writes, and the examples' `docker-compose.yml`, map `"<port>:8000"`. Docker binds
   that on `0.0.0.0`, so other machines on the network can reach it too.
3. **Plaintext data on disk.** The Java and Docker setups run with `-sharedDb -dbPath <dir>`.
   The data is a SQLite file in that folder (or the Docker volume), not sealed.

Whether this data is "in dqlrs" depends on the person: local tables often hold copies of real
data for testing.

## Reproduced

From the code: `dynamo.rs:621` passes `-sharedDb -dbPath`, and `dynamo.rs:675`
`generated_compose` writes `ports: - "{port}:8000"`. `dqlrs-web/examples/docker-compose.yml`
maps `"8000:8000"`.

## Fix

- Publish on loopback only: `"127.0.0.1:{port}:8000"` in both compose files.
- Say in the docs that DynamoDB Local has no access control and that its data folder is
  plaintext. Offer `-inMemory` as the default for scratch work.
- Treat local data like remote data in the model: an agent reaches it only through grants. That
  holds only if the agent cannot reach the port, which brings back the sandbox in the README.
