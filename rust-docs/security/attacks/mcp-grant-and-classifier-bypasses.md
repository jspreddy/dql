# MCP Grant Preview Truncation, Classifier Blind Spots, and Shared Grant Scope

## Summary

Within the legitimate MCP workflow (`request_access` -> human review -> `read_granted_table`), four design gaps allow a purely MCP-confined agent to obtain uninspected sensitive rows, evade sensitivity warnings in the approval UI, read grants approved for a different agent, or infer non-key columns from ungranted virtual tables.

## Violated boundary

- **Intended:** When a human reviews and approves a `request_access` prompt, the UI accurately reflects the rows and sensitivity of what is being granted, and the resulting grant is scoped to the requesting agent.
- **Actual:** The human previews at most 20 rows and the classifier scans at most 1,000 rows while the grant exposes the **entire** result set; binary values bypass classification; grants are globally readable by every MCP caller; and virtual-table metadata reveals data-derived column names.

## Code paths

1. **Preview and classifier truncation (`20` / `1,000` vs. unbounded grant):**
   - In `rust-impl/crates/dql-cli/src/web/server.rs` (`run_access`), `dqlrs` executes `CREATE TEMPTABLE mcp_r<N> AS (<query>)`, storing **every** matching row in memory.
   - `run_access` then fetches only `state.session.engine.temp_table_page(&table, 0, 1000)` (`SCAN_CAP = 1000` in `web/classify.rs`), runs `classify::classify(&page, total)` on those first 1,000 rows, and slices `.take(20)` for the `rows` preview rendered in the browser (`renderGrantRows` in `dqlrs-web/static/app.js`).
   - When the human clicks **Grant**, `grant_access` executes `GRANT MCP READ ON TEMPTABLE mcp_r<N>`, which exposes **all `total` rows** to `read_granted_table` (`offset` has no upper bound in `mcp.rs` / `virtual_tables.rs`).
2. **Sensitivity classifier blind spots (`rust-impl/crates/dql-cli/src/web/classify.rs`):**
   - `walk_value` explicitly ignores binary values:
     ```rust
     Value::Null | Value::Binary(_) | Value::Timestamp(_) | Value::Interval(_) => {}
     ```
     Any sensitive data stored in DynamoDB `B` (Binary) attributes—such as UTF-8 JSON documents, PEM keys, JWTs, or serialized records—never runs through `note_values`.
   - `name_pattern` checks 1–3 camel/snake/dot-split words against a static `NAME_PATTERNS` list. Generic attribute names (`payload`, `body`, `data`, `notes`, `meta`, `v`, `col1`) produce zero attribute hits, and values that do not match the fixed regexes (email, SSN, Luhn card, US phone, AWS key, `sk-*`, `Bearer`, PEM header, JWT) produce **"No sensitive attributes or values found."**
3. **Global grant pool and spoofable `agent` identity:**
   - In `rust-impl/crates/dql-cli/src/mcp.rs` (`request_access`), the `agent` label displayed in the web grant panel comes from the tool call's optional `"agent"` argument (falling back to the self-declared `clientInfo.name` from MCP `initialize`). Any agent can pass `"agent": "schema-helper"` (or another trusted agent's name) to spoof who is asking.
   - `App` shares a single `Session` and a single `access::Store` across all `/mcp` callers. Neither `request_access(request: "r1")`, `list_granted_tables`, nor `read_granted_table` binds a request or a granted temp table (`mcp_r1`) to the specific MCP session or agent that requested it.
4. **Data-derived column inference on ungranted virtual tables:**
   - Unlike DynamoDB's `DescribeTable` API (which returns only key and index `AttributeDefinitions`, hiding schemaless non-key item attributes), `VirtualCatalog` (`rust-impl/crates/dql-engine/src/virtual_tables.rs`) infers `TableMeta` columns for `TEMPTABLE` and `LOCALTABLE` by scanning **every attribute key across all rows** (`columns_for_saved_rows`, `infer_columns`).
   - Calling `list_tables` or `describe_tables` on `/mcp` exposes every ungranted human `TEMPTABLE` and `LOCALTABLE` name—plus every column name present in their underlying rows—and triggers `reload_local` disk reads on CSV/JSON local tables.

## Circumvention mechanics

1. **Hiding sensitive rows past row 20 or row 1,000:**
   - An agent submits `request_access` with a `SELECT` or `SCAN` (using `ORDER BY`, an index scan order, or `JOIN` against another table) where the first 20 rows (or first 1,000 rows) contain benign placeholder/public records and rows `>= 1,000` contain sensitive records.
   - If sensitive rows sit at index `>= 1,000`, `classify` reports `attributes: []`, `values: []` (**"No sensitive attributes or values found"**) and the human sees 20 clean rows. Even if sensitive rows sit at index `20..999` under generic column names (`notes`, `payload`) without matching regexes, the human's 20-row table preview never displays them.
   - Once granted, the agent calls `read_granted_table(name: "mcp_r1", offset: 1000, limit: 1000)` to retrieve the uninspected tail of the table.
2. **Exfiltrating binary attributes without warnings:** Requesting `SELECT id, blob FROM ...` where `blob` is a DynamoDB `Binary` attribute containing sensitive UTF-8 text or credentials triggers neither attribute nor value classification, presenting a clean badge to the reviewer while `read_granted_table` returns the base64-encoded binary payload.
3. **Piggybacking on another agent's grants:** When multiple agents or MCP clients run on the same workstation, a secondary agent can call `list_granted_tables` and `read_granted_table` (or poll `request_access` with `request: "r1"`, `"r2"`, ...) to read data the human granted to a completely different agent task.

## Remediation

- **Bind grants strictly to what was classified and previewed:**
  - Either enforce a hard row cap on `request_access` queries (for example `<= 1,000` rows, requiring explicit pagination in the UI to inspect all rows) **and** run `classify` over **100%** of rows stored in `mcp_r<N>`, or truncate `mcp_r<N>` to the exact slice classified and allow the human to page through all rows in the grant panel before clicking **Grant**.
- **Classify `Value::Binary` payloads:** Decode `Value::Binary` bytes (both raw UTF-8/ASCII substrings and base64) in `walk_value` so secrets, emails, JWTs, and keys stored in binary attributes are flagged, and surface an explicit informational note whenever binary or opaque attributes are present.
- **Isolate grants per authenticated MCP client session:** Issue distinct per-client MCP tokens (or `Mcp-Session-Id` bound to distinct tokens), tie `agent` identity to the server-registered client configuration rather than caller-supplied tool arguments, and scope `request_access` IDs, `list_granted_tables`, and `read_granted_table` to the specific principal that received the grant.
- **Hide ungranted temp tables from MCP metadata:** Filter ungranted `TableKind::Temp` tables (or their non-key data-inferred columns) out of `/mcp` `list_tables` and `describe_tables` so human scratch queries in `dqlrs web` do not leak row-derived attribute names to agents.
