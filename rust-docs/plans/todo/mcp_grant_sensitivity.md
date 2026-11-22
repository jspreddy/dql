# Classify the result before the user grants it

**Priority:** Medium  
**Area:** `dqlrs web` Run path, `dqlrs-web` grant panel  
**Status:** Not started. Step 4 of [`mcp_grant_flow.md`](mcp_grant_flow.md).
Run and the decision are [`mcp_grant_panel.md`](mcp_grant_panel.md). The grant
event is [`mcp_grant_events.md`](mcp_grant_events.md).

## Flow

1. **Request.** The agent calls `request_access` with one read query. [`mcp_grant_panel.md`](mcp_grant_panel.md)
2. **Inspect.** The user reads that query.
3. **Run.** The user runs it. The result is a temp table, `mcp_<id>`.
4. **Classify.** This file. Attribute name classifiers and value classifiers run on that temp table. The panel shows the rows from Run together with both match lists.
5. **Grant.** The user grants or denies that same temp table. A match recommends not granting. The user decides. This file does not grant.

## Why

Run has produced rows and the user is about to hand them to the agent. The
panel can show the rows and still not say which column names or which values
look sensitive. Classify runs on the temp table the user just produced, and
the recommendation sits with those rows before Grant is offered.

## Decisions

| Question | Decision |
| --- | --- |
| Classifiers | Exactly two. Attribute name classifiers, and value classifiers. Both are shown, and both feed one recommendation |
| Attribute name classifiers | Patterns on attribute names, including the keys of nested maps (`profile.email`). Generic: the same patterns for every temp table, including datasets that do not exist yet. Not taken from one table's schema |
| Value classifiers | Run on the values. A fixed list in code, like the name list. Unlike the name list, an entry does not have to suit every dataset: one may target a shape known in this data, such as one ID format |
| When | As part of Run, on `mcp_<id>`, before Grant is enabled. Run again classifies the replacement table. Nothing is classified at request time, because the query has not run |
| What is scanned | That temp table's rows in memory (`MemoryBackend::table_page`). The query is not run a second time |
| How much | The first 1000 rows. The lines name how many were scanned and how many the table has |
| Which values | Every attribute of those rows. Strings, numbers rendered as text, and the leaves of lists and maps. Nulls are skipped |
| What the user is shown | Next to the result rows: the attribute names that matched, and the values that matched, each with its classifier. A long list names the first few distinct values, then a count of the rest |
| Recommendation | Any match from either classifier: recommend not granting. No matches: say so, with how many rows were scanned. Patterns miss things, so no match is not called safe. Grant stays the user's choice in the panel |
| Stale result | If that temp table is dropped before Grant, the matches are dropped with it and Grant stays off until the user runs again |
| Who sees it | The panel, beside the rows. The MCP client gets the table name or the denial only. The grant event carries no matches |

## Attribute name classifiers

One list, used for every temp table. A new dataset does not add a pattern and
does not drop one. A name is split into lower-case words at `_`, `-`, `.`, and
camelCase. A pattern matches one word, or two or three adjacent words joined
(`credit_card` gives `creditcard`). Plain substrings are not used: they would
match `dob` in `adobe_id` and `token` in `tokens_used`.

The patterns:

`email`, `phone`, `ssn`, `password`, `passwd`, `secret`, `token`, `apikey`,
`creditcard`, `cardnumber`, `cvv`, `dob`, `birthdate`, `address`, `passport`.

`user_email` and `userEmail` match `email`. `api_key` matches `apikey`.
`order_id` and `adobe_id` match nothing. The finding names
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

Returned with the Run result and stored on the pending request.

| Findings | Shown with the rows |
| --- | --- |
| Attribute names, values, or both | Recommend not granting. Attributes: `user_email` (`email`). Values: `ada@example.com` in `notes` (`email`) |
| None, table smaller than the cap | No sensitive attributes or values found in 40 rows |
| None, table larger than the cap | No sensitive attributes or values found in the first 1000 of 8000 rows |

If the scan cannot read the table, Run shows that error and the panel leaves
Grant off.

## Where it hooks

- The classifiers are plain functions in `dql-cli` (for example
  `web/classify.rs`): rows in, findings out. No DOM, no engine change, so unit
  tests cover them directly.
- Read pages from `MemoryBackend::table_page` in `memory.rs`. The temp table
  is the one Run just created in `virtual_tables.rs`.
- The panel calls this at the end of Run and renders both lists beside the
  rows. Grant still runs `GRANT MCP READ ON TEMPTABLE mcp_<id>`.

## Out of scope

- Granting or denying. That decision is the panel.
- Sending matches to the MCP client or on `grant_changed`.
- Classifying at request time, or classifying a table Run did not just create.
- A third classifier, a model, or a network call.
- Building the attribute name list from the result's schema.
- Requiring value classifiers to apply to every future dataset.

## Steps

1. Attribute name classifiers: one generic pattern list, applied to every
   attribute name of the Run result, nested keys included. Tests: `user_email`
   and `userEmail` match, `order_id` and `adobe_id` do not, and a second result
   with the same name uses the same list.
2. Value classifiers: every attribute value in the first 1000 rows, including
   numbers and nested leaves. Tests: an email value is named, a Luhn card is
   named, a clean result says nothing was found and how many rows were scanned.
3. The panel shows those lines with the rows, and Grant stays off until they
   are present. The end-to-end check is the Playwright test in the panel plan.
