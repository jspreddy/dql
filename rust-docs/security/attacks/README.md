# Attacks on the agent data-access model

**Status:** nothing here is fixed yet. This folder reconciles two independent reviews of `v-rust`
at `0943b17` (2026-11-27):

| Reviewer | Pull request | Label in this folder |
| --- | --- | --- |
| Cursor Agent | `cursor/cli-security-review-86d7` (#336) | Cursor |
| Claude Code | `security/attack-review` (#337) | Claude |

Each file names who found each point. "Both" means both reviews reported it independently.
Where the reviews disagreed, the file has a **Reconciled** section and the table below lists it.

## The model these attacks break

| Who | Rows | Metadata (tables, keys, indexes, attribute names) | Surfaces |
| --- | --- | --- | --- |
| A person | Yes | Yes | CLI and REPL, `dqlrs notebook`, `dqlrs web`, the grant panel |
| An agent | Only rows the person granted | Yes | `/mcp` only |

An agent may read metadata, create, read, update, lint, and search `.dql` files, ask for access
with `request_access`, and read a temp table after the person grants it. It must not run a query,
grant itself access, or read ungranted rows from DynamoDB, the caches, the API, worker processes,
notebooks, DynamoDB Local, or MCP. This holds for any agent on the machine, not only the one the
person configured. Data the person explicitly saved (`SAVE`, an export) is out of scope. Caches,
runtime files, local databases, and config that `dqlrs` manages are in scope.

The reference setup vends AWS credentials only to `dqlrs`: `aws-vault exec <profile> -- dqlrs …`.

## The root cause

The shipped design ([`plans/archive/security/`](../../plans/archive/security/README.md)) treats a
process running as the same OS user, or able to read the MCP config, the terminal, or
`http://127.0.0.1:8765/`, as the person. Every boundary that exists (the API token, sealed caches,
`0700`/`0600` modes) stops other OS users. None separates a person from an agent that runs as that
person:

- the running `dqlrs` holds the credentials and answers anyone with its token (01, 02, 12);
- that token is handed out freely and is the agent's MCP token too (01, 02);
- the token and the cache key sit in stores any process of the user can read (03);
- agent-written `.dql` files run with the person's rights (06, 07);
- the person's credentials can be read from, or asked of, the processes that hold them (14, 15).

So the grant flow is advisory: it holds only for an agent that uses nothing but its MCP tools, and
even then the person's Run button and the grant panel can be turned against them (06, 07, 09).

## Index

**Severity** is against the model: **Critical** means an agent reads arbitrary rows with no person
involved; **High** means it reads rows with a person's unwitting help, or reads stored rows;
**Medium** and **Low** are narrower. **Needs** is the least an agent must have. **Under
`aws-vault`** says what changes when credentials are vended only to `dqlrs`.

| # | Attack | Severity | Needs | Under `aws-vault` | Found by |
| --- | --- | --- | --- | --- | --- |
| 01 | [The page serves the token without auth; no Host check](01-page-serves-the-token.md) | Critical | Loopback HTTP, or a web page through DNS rebinding | Unchanged | Both; DNS rebinding Claude |
| 02 | [The MCP token is the API token](02-shared-token-api-escalation.md) | Critical | The MCP config | Unchanged | Both; kill switch, TCP `--serve`, dot-folder reads Cursor; script grant Claude |
| 03 | [Token and cache key in stores any user process reads](03-token-and-cache-key-in-user-stores.md) | Critical | A shell | Unchanged | Both; key-path overrides Cursor |
| 04 | [The agent runs `dqlrs` or the AWS SDK itself](04-agent-runs-dqlrs-or-the-aws-sdk.md) | Critical | A shell | Narrowed to 13 to 16 | Both |
| 05 | [Cached query results, decrypted](05-result-cache-decryption.md) | High | A shell | Unchanged via the key; second-server route limited | Both; connection-folder limit Cursor |
| 06 | [Query-file poisoning: agent-written files the person runs](06-query-file-poisoning.md) | High | MCP file tools, and the person pressing Run | Unchanged | Both; Run folder and revision race Cursor |
| 07 | [Statement smuggling through the splitter](07-statement-splitter-smuggling.md) | High | MCP file tools, and the person pressing Run | Unchanged | Cursor |
| 08 | [A granted temp table can change after the grant](08-granted-table-changes-after-grant.md) | Medium | A write to the table | Unchanged | Claude |
| 09 | [Grant panel shows 20 rows, classifies 1,000, grants all](09-grant-preview-and-classifier-gaps.md) | High | MCP only, and a person who grants | Unchanged | Cursor |
| 10 | [Grants shared by every MCP client; self-declared agent name](10-shared-grant-pool-and-agent-label.md) | Medium | A second MCP client | Unchanged | Cursor |
| 11 | [Virtual-table metadata comes from the rows](11-metadata-from-virtual-tables.md) | Low | MCP only | Unchanged | Cursor |
| 12 | [Notebook: tokens, kernel keys, kernel code, autosaved outputs](12-notebook-runtime-and-outputs.md) | High | A shell, or workspace reads for outputs | Worse: terminals inherit credentials | Both; mutable kernel code Cursor |
| 13 | [DynamoDB Local: no auth, all interfaces, plaintext](13-dynamodb-local.md) | Medium | Network or workspace access | Unchanged | Both; all interfaces Claude; workspace path, registry, skip list Cursor |
| 14 | [AWS credentials in the process environment](14-credentials-in-process-environment.md) | Critical on Linux | A shell | Where the credentials are | Both; reproduced by Claude |
| 15 | [The agent runs `aws-vault` itself](15-agent-runs-aws-vault.md) | Critical, or blocked by setup | A shell | Depends on the backend | Both |
| 16 | [Replacing `dqlrs` or a startup file](16-binary-and-startup-file-shadowing.md) | High | A shell, and the person's next run | How fresh credentials reach agent code | Cursor |
| 17 | [REPL history and the keychain secret in `argv`](17-history-and-small-leaks.md) | Low | A shell, or another user's `ps` | Unchanged | Both; `argv` Claude; file modes Cursor |

Paths that need **no shell at all**: 01 (loopback HTTP), 02 (the MCP config), 06, 07, 09, 10, 11
(MCP tools). Those are the ones fixes inside `dqlrs` can close completely.

## Where the reviews disagreed

| Topic | Cursor said | Claude said | Resolution |
| --- | --- | --- | --- |
| Web pages reading `GET /` | The same-origin policy blocks remote sites | DNS rebinding makes the page same-origin, and `Host` is never checked | Claude; reproduced with `Host: attacker.example` (01) |
| MCP config files | `dqlrs` writes the token into `.cursor/mcp.json` and `.mcp.json` | `dqlrs` prints and copies the snippet | No code writes those files; the person pastes it. Same risk (02) |
| Docker DynamoDB Local | Published on `127.0.0.1:<port>` | Published on all interfaces | `"{port}:8000"` in `dynamo.rs:675` and the examples: all interfaces (13) |
| A second `dqlrs web` reading the cache | Cannot attach to an AWS connection's folder under `aws-vault` | Reads the history (reproduced on the memory backend) | Both: the route works for memory and DynamoDB Local; for AWS the key still decrypts (05) |
| `/proc/<pid>/environ` | Needs `ptrace_scope = 0` | Readable by the same user (reproduced on a kernel without Yama) | Yama limits attach, not the read check, so expected readable at `ptrace_scope = 1`. **To confirm** on stock Ubuntu (14) |
| macOS process environment | SIP blocks it | `ps -E` likely shows it | Neither ran it. **To confirm** with `ps -E -p <pid>` (14) |

## Checked and held

From Claude's review; Cursor's review did not list held checks.

- **`request_access` injection.** `Store::check_query` (`web/access.rs:64`) parses exactly one
  `SELECT` or `SCAN` and rejects `SAVE`, before the query is wrapped in `CREATE TEMPTABLE … AS (…)`.
  (07 reaches the runner by a different path: a human-run script, not `request_access`.)
- **MCP tools return metadata only.** Lint, search, list, and describe read cached definitions,
  names, and `.dql` text, and execute nothing. 11 notes that some of that metadata is row-derived.
- **`read_granted_table`** refuses a table without a live grant; recreating a table ends its grant.
  See 08 for writes and 09 for what a grant covers.
- **MCP file tools** stay in the workspace and accept only `.dql`. A `SAVE` to `x.dql` writes bytes
  that are not valid UTF-8, so `read_query_file` refuses it; that is incidental, not designed.
- **Script injection in the web UI.** The one `innerHTML` sink escapes Markdown and filters links;
  Mermaid runs `securityLevel: "strict"`; HTML documents render in a sandboxed iframe without
  `allow-same-origin`.
- **Token comparison** is constant-time; listen addresses are loopback only; `/mcp` rejects a
  non-loopback `Origin`.
- **Other OS users** cannot read the sealed caches or call the API without the token.

## What would close the gap

Both reviews reach the same conclusion: no change inside `dqlrs` alone stops a process that runs as
the person and can reach the person's credentials (04, 14, 15, 16). The model needs one of:

1. **A different identity for agents** (both): a sandbox, container, or separate OS user with no
   access to `aws-vault`, the login key store, `/proc` of the person's processes, `~/.local`, shell
   startup files, or loopback ports other than `/mcp`. [`SANDBOX.md`](../../../SANDBOX.md) is a
   start.
2. **A person-presence check** (Claude): an OS prompt (Touch ID, Windows Hello, polkit) guarding the
   key store and the grant, which a background process cannot answer.

Inside `dqlrs`, in priority order:

1. Separate credentials: a per-process browser session from a one-time launch nonce, never served
   by `GET /` or printed; MCP tokens accepted only on `/mcp`, per client; approvals only from the
   browser session (01, 02, 10).
2. Check `Host`, `Origin`, and `Sec-Fetch-Site` on every route (01).
3. Grants only from the grant panel: no `GRANT MCP READ` in scripts; parse with `dql-parser` and
   run one statement per step; show side effects and pin revisions before a run (06, 07).
4. Grant only what the person saw: cap and classify every row, classify binary values, freeze a
   granted table (08, 09).
5. Make `dqlrs` non-dumpable at startup and strip `AWS_*` from its children; turn off Jupyter
   terminals (12, 14).
6. Stop writing rows where a same-user process can read them without the person: result history
   on disk, notebook autosave, DynamoDB Local in the workspace (05, 12, 13).
7. Install `dqlrs` and the notebook kernel read-only (12, 16); publish DynamoDB Local on loopback
   only (13); seal REPL history (17).
