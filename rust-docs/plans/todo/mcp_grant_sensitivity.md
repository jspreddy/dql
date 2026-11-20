# Recommend for or against an MCP grant

**Priority:** Medium  
**Area:** `dql-engine` temp tables, `dqlrs-web` grant panel  
**Status:** Not started. The pending row is the one in
[`mcp_grant_panel.md`](mcp_grant_panel.md). This plan adds the recommendation
on that row. It does not change Approve or Deny.

## Why

A pending `request_grant` names a temp table and waits. The panel shows the
name, the access (`MCP read`), and who asked. It does not say whether the
columns or the cells look sensitive. Two classifiers run before the user
grants. The request row names the attributes and the values that might be
sensitive, and recommends not granting when either classifier matches.

## Decisions

| Question | Decision |
| --- | --- |
| Classifiers | Exactly two. Attribute name classifiers, and value classifiers. Both feed the same recommendation |
| Attribute name classifiers | Patterns on column names. Generic: the same patterns for every temp table, including datasets that do not exist yet. Not taken from one table's schema |
| Value classifiers | Run on the values. They do not have to be generic. A value classifier may be specific to the data in the table being requested |
| When | Once, as the server records `request_grant`, before the row is shown. Again if that temp table is replaced (`CREATE TEMPTABLE` of the same name, new generation) while the request is still pending |
| What is scanned | The temp table's rows already in memory (`MemoryBackend::table_page`). The query is not run again, and DynamoDB is not called |
| How much | The first 1000 rows. The reason names how many were scanned and how many the table has |
| Which values | Every attribute of those rows. Strings, numbers rendered as text, and the leaves of lists and maps. Nulls are skipped |
| What the user is shown | The attribute names that matched, and the values that matched, each with its classifier. A long list names the first few distinct values, then a count of the rest |
| Recommendation | Any match from either classifier: recommend not granting. No matches: recommend granting. The user still Approves or Denies. The server does not grant or deny from the scan |
| Missing table | No recommendation. The row keeps the error from the panel plan, and `GRANT` still fails until the temp table exists |
| Who sees it | The web panel, on that pending request. The MCP client gets success or denied only, the same as the panel plan. The grant event stream is unchanged |

## Attribute name classifiers

One list, used for every temp table. A new dataset does not add a pattern and
does not drop one. Names are compared case-insensitively with `_` and `-`
removed. A match is the whole name or a name that contains one of these:

`email`, `phone`, `ssn`, `password`, `passwd`, `secret`, `token`, `apikey`,
`creditcard`, `cardnumber`, `cvv`, `dob`, `birthdate`, `address`, `passport`.

`user_email` matches `email`. `order_id` matches nothing. The finding names
the attribute (`user_email`) and the pattern (`email`). It does not depend on
the cell values.

## Value classifiers

Each scanned value is passed through the value classifiers. These classifiers
are allowed to be specific to the data at hand. They are not required to work
for a later dataset.

| Classifier | Match |
| --- | --- |
| `email` | `name@host.tld` |
| `phone` | A US phone shape |
| `ssn` | `###-##-####` |
| `card` | 13–19 digits that pass the Luhn check |
| `aws-key` | `AKIA` plus 16 letters or digits |
| `token` | A single run of 32 or more token characters |

A finding names the attribute, the classifier, and the value
(`ada@example.com` in `notes`, classifier `email`). One value can match more
than one classifier. The scan still reads up to 1000 rows after the first
match, so the named values are the ones inside that window.

## Recommendation

Computed on the server and stored on the pending request. Both classifier
results are in the line.

| Findings | Line on the request |
| --- | --- |
| Attribute names, values, or both | Recommend not granting. Attributes: `user_email` (`email`). Values: `ada@example.com` in `notes` (`email`) |
| None, table smaller than the cap | Recommend granting — no sensitive attributes or values in 40 rows |
| None, table larger than the cap | Recommend granting — no sensitive attributes or values in the first 1000 of 8000 rows |

A scan that cannot read the table shows "No recommendation" and leaves
Approve and Deny as they are.

## Where it shows

On the pending row from `mcp_grant_panel.md`, beside Approve and Deny:

| Agent | Table | Access | Asked | Recommendation |
| --- | --- | --- | --- | --- |
| `M1` | `late_orders` | MCP read | the request time | Recommend not granting. Attributes: `user_email` (`email`). Values: `ada@example.com` in `notes` (`email`) |

`GET /api/mcp-access` and the `grant-request` browser event carry that line
with the request, so the panel does not scan and does not poll. Granted rows
stay as the panel plan describes them. Revoke stays a statement.

## Where it hooks

- Read pages from `MemoryBackend::table_page` in `memory.rs`. Temp-table rows
  live on the catalog's memory backend in `virtual_tables.rs`.
- Attach both classifier results to the pending `request_grant` in the web
  server, next to the request the panel plan stores.
- Render the line in the grant panel. Approve still runs
  `GRANT MCP READ ON TEMPTABLE`.

## Out of scope

- Granting or denying without the user.
- Sending findings, or the recommendation, to the MCP client.
- Scanning a DynamoDB table or a local table.
- A third classifier, a model, or a network call.
- Building the attribute name list from the temp table's schema.
- Requiring value classifiers to apply to every future dataset.
- A recommendation on a grant that was already made.

## Steps

1. Attribute name classifiers: one generic pattern list, applied to every
   column of any temp table. Tests: `user_email` matches, `order_id` does not,
   and a second table with the same column name uses the same list.
2. Value classifiers: every attribute value in the first 1000 rows, including
   numbers and nested leaves. Tests: an email value is named, a Luhn card is
   named, a clean table recommends granting, and a value classifier is not
   required to be reused on a different dataset.
3. Store both results on `request_grant`, including after a generation change.
   The line names the attributes and the values, and recommends not granting
   when either side matches.
4. Show it on the pending row. A Playwright test: the row names `user_email`
   and `ada@example.com` and says recommend not granting, a table of order ids
   says recommend granting, and Approve still grants after a
   recommend-not-granting line.
