// Golden cases for table browser reads: a read and the table it names, and the statement and
// message they build. The JavaScript builder must match until the server builds reads
// (rust-docs/plans/todo/table_reads_server.md). Cases marked "fixes" are bugs in this builder
// that the Rust builder fixes; they are skipped here.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { buildReadStatement, readError } from "../static/table-query.js";

const here = path.dirname(fileURLToPath(import.meta.url));
const fixture = path.join(here, "../../rust-impl/crates/dql-cli/tests/fixtures/table_reads.json");
const cases = JSON.parse(fs.readFileSync(fixture, "utf8"));

test("the golden file has cases", () => {
  assert.ok(cases.length >= 50, `only ${cases.length} cases`);
  for (const item of cases) {
    assert.equal(item.read.table ?? item.table.name, item.table.name, item.name);
  }
});

for (const item of cases) {
  test(`golden: ${item.name}`, { skip: item.fixes ? `fixed by the Rust builder: ${item.fixes}` : false }, () => {
    const extra = new Map((item.read.extra || []).map((name) => [name, ""]));
    const state = { ...item.read };
    const error = readError(item.table, state, extra);
    assert.equal(error, item.error);
    if (!item.error) {
      assert.equal(buildReadStatement(item.table.name, item.table, state, extra), item.dql);
    }
  });
}
