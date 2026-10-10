import assert from "node:assert/strict";
import test from "node:test";

import { parseLsMessage, parseTableDescription, parseTableSummary } from "../static/describe-card.js";

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

test("parseLsMessage keeps an intelligent-match note on a description and a list", () => {
  const note = 'No exact match for "post", so showing intelligent matches.';
  const one = parseLsMessage(`${note}\n\n${detail}`);
  assert.equal(one.note, note);
  assert.equal(one.description.name, "posts");
  assert.equal(one.summary, "");

  const summary = [
    "Tables",
    "Name                         Items     Read    Write     Status     Size",
    "posts_v2                         0        -        -     ACTIVE     0 B",
    "nb_posts                         0        2        3     ACTIVE   1.5 KiB",
  ].join("\n");
  const many = parseLsMessage(`${note}\n\n${summary}`);
  assert.equal(many.note, note);
  assert.equal(many.description, null);
  assert.match(many.summary, /^Tables\n/);
  assert.deepEqual(many.tables, [
    { name: "posts_v2", items: "0", read: "-", write: "-", status: "ACTIVE", size: "0 B" },
    { name: "nb_posts", items: "0", read: "2", write: "3", status: "ACTIVE", size: "1.5 KiB" },
  ]);

  const exact = parseLsMessage(detail);
  assert.equal(exact.note, "");
  assert.equal(exact.description.name, "posts");
  assert.deepEqual(exact.tables, []);
});

test("parseTableSummary reads size text and rejects a description", () => {
  assert.equal(parseTableSummary(detail), null);
  assert.equal(parseTableSummary("Tables\nName Items\nposts 0"), null);
});
