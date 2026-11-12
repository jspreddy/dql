import { test, expect } from "../../support/app.js";
import { openComponentHarness } from "../../support/components.js";

// statement-text.js: result labels, read/write classification, and run counts.

test.beforeEach(async ({ page, app }, testInfo) => {
  await openComponentHarness(page, app, testInfo);
});

test("labels results by the statement verb, past leading comments", async ({ page }) => {
  const labels = await page.evaluate(async () => {
    const { resultLabel } = await import("/statement-text.js");
    const affected = (statement, affected) => resultLabel({ ok: true, kind: "affected", statement, affected });
    return {
      inserted: affected("INSERT INTO t (id) VALUES ('a'), ('b');", 2),
      one: affected("UPDATE t SET n = 1 WHERE id = 'a';", 1),
      deleted: affected("-- clean up\n-- twice\nDELETE FROM t WHERE id = 'b';", 1),
      loaded: affected("LOAD 'rows.json' INTO t;", 3),
      saved: affected("SCAN * FROM t SAVE 'out.json';", 2),
      count: affected("SCAN count(*) FROM t;", 1),
      other: affected("EXPLAIN SCAN * FROM t;", 0),
      dropped: resultLabel({ ok: true, kind: "status", statement: "-- bye\nDROP TABLE t;", message: "Dropped table 't'" }),
      missing: resultLabel({ ok: true, kind: "status", statement: "DROP TABLE IF EXISTS t;", message: "Table 't' did not exist" }),
      rows: resultLabel({ ok: true, kind: "items", items: [{}, {}] }),
      failed: resultLabel({ ok: false, error: "boom" }),
    };
  });
  expect(labels).toEqual({
    inserted: "2 rows inserted",
    one: "one row updated",
    deleted: "one row deleted",
    loaded: "3 rows loaded",
    saved: "2 rows saved",
    count: "one row",
    other: "0 affected",
    dropped: "one table dropped",
    missing: "Table 't' did not exist",
    rows: "2 rows",
    failed: "boom",
  });
});

test("classifies reads and writes and totals run counts", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { addResultCounts, emptyCounts, fileSummaryText, statementAccess, isSaveStatement } = await import("/statement-text.js");
    const access = Object.fromEntries(
      [
        "SELECT * FROM t WHERE id = 'a'",
        "SCAN * FROM t",
        "SCAN * FROM t SAVE 'out.json'",
        "INSERT INTO t (id) VALUES ('a')",
        "CREATE TABLE t (id STRING HASH KEY)",
        "DROP TABLE t",
        "ALTER TABLE t SET THROUGHPUT (1, 1)",
        "-- note\nLOAD 'x.json' INTO t",
        "DESCRIBE t",
      ].map((statement) => [statement, statementAccess(statement)]),
    );
    const counts = emptyCounts();
    for (const item of [
      { statement: "SCAN * FROM t", ok: true, read_units: 0.5 },
      { statement: "INSERT INTO t (id) VALUES ('a')", ok: true, write_units: 1 },
      { statement: "SELEKT nope", ok: false },
    ]) addResultCounts(counts, item);
    return {
      access,
      counts,
      summary: fileSummaryText(counts),
      saveInString: isSaveStatement("SELECT * FROM t WHERE note = 'save me'"),
    };
  });
  expect(result.access).toEqual({
    "SELECT * FROM t WHERE id = 'a'": { read: true, write: false },
    "SCAN * FROM t": { read: true, write: false },
    "SCAN * FROM t SAVE 'out.json'": { read: true, write: true },
    "INSERT INTO t (id) VALUES ('a')": { read: false, write: true },
    "CREATE TABLE t (id STRING HASH KEY)": { read: false, write: true },
    "DROP TABLE t": { read: false, write: true },
    "ALTER TABLE t SET THROUGHPUT (1, 1)": { read: false, write: true },
    "-- note\nLOAD 'x.json' INTO t": { read: false, write: true },
    "DESCRIBE t": { read: false, write: false },
  });
  expect(result.counts).toEqual({ queries: 3, reads: 1, writes: 1, success: 2, failure: 1, readUnits: 0.5, writeUnits: 1 });
  expect(result.summary).toBe("3 queries · one read · one write · 2 ok · 1 failed");
  expect(result.saveInString).toBe(false);
});
