import assert from "node:assert/strict";
import test from "node:test";

import { parseTableDescription } from "../static/describe-card.js";

const detail = `Name: posts
Status: ACTIVE
Items: 0
Size: 0 B
Read: N/A
Write: N/A
Hash Key: id (STRING)
Range Key: n (NUMBER)

Global Indexes:
  Name             Projection

CREATE TABLE posts (id STRING HASH KEY, n NUMBER RANGE KEY, THROUGHPUT (0, 0));`;

test("parseTableDescription splits keys, stats, and the CREATE query", () => {
  const parsed = parseTableDescription(detail);
  assert.equal(parsed.name, "posts");
  assert.equal(parsed.status, "ACTIVE");
  assert.equal(parsed.items, "0");
  assert.equal(parsed.read, "N/A");
  assert.deepEqual(parsed.hashKey, { name: "id", type: "STRING" });
  assert.deepEqual(parsed.rangeKey, { name: "n", type: "NUMBER" });
  assert.match(parsed.extra, /Global Indexes:/);
  assert.match(parsed.schema, /^CREATE TABLE posts /);
  assert.match(parsed.schema, /THROUGHPUT \(0, 0\)/);
});

test("parseTableDescription ignores list output and plain status text", () => {
  assert.equal(parseTableDescription("Tables\nName  Items\nposts  0"), null);
  assert.equal(parseTableDescription("Created table 'posts'"), null);
  assert.equal(parseTableDescription("Name: posts\nStatus: ACTIVE"), null);
});
