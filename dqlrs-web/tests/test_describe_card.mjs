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

Local Indexes:
  ts-index  hash=id  range=n  projection=ALL
  views-index  hash=id  range=views  projection=INCLUDE(title,author)

Global Indexes:
  Name             Projection       Read    Write HashKey              RangeKey               Status
  by-author        ALL              10/20 (50%)        3 author (STRING)      n (STRING)             ACTIVE
  by-id            KEYS                 N/A      7/∞ id (STRING)          -                      ACTIVE

Note that is not an index

CREATE TABLE posts (id STRING HASH KEY, n NUMBER RANGE KEY, THROUGHPUT (0, 0));`;

test("parseTableDescription splits keys, stats, and the CREATE query", () => {
  const parsed = parseTableDescription(detail);
  assert.equal(parsed.name, "posts");
  assert.equal(parsed.status, "ACTIVE");
  assert.equal(parsed.items, "0");
  assert.equal(parsed.read, "N/A");
  assert.deepEqual(parsed.hashKey, { name: "id", type: "STRING" });
  assert.deepEqual(parsed.rangeKey, { name: "n", type: "NUMBER" });
  assert.deepEqual(parsed.localIndexes, [
    { name: "ts-index", hash: "id", range: "n", projection: "ALL" },
    { name: "views-index", hash: "id", range: "views", projection: "INCLUDE(title,author)" },
  ]);
  assert.deepEqual(parsed.globalIndexes, [
    {
      name: "by-author",
      projection: "ALL",
      read: "10/20 (50%)",
      write: "3",
      hashKey: { name: "author", type: "STRING" },
      rangeKey: { name: "n", type: "STRING" },
      status: "ACTIVE",
    },
    {
      name: "by-id",
      projection: "KEYS",
      read: "N/A",
      write: "7/∞",
      hashKey: { name: "id", type: "STRING" },
      rangeKey: null,
      status: "ACTIVE",
    },
  ]);
  assert.equal(parsed.extra, "Note that is not an index");
  assert.match(parsed.schema, /^CREATE TABLE posts /);
  assert.match(parsed.schema, /THROUGHPUT \(0, 0\)/);
});

test("parseTableDescription ignores list output and plain status text", () => {
  assert.equal(parseTableDescription("Tables\nName  Items\nposts  0"), null);
  assert.equal(parseTableDescription("Created table 'posts'"), null);
  assert.equal(parseTableDescription("Name: posts\nStatus: ACTIVE"), null);
});

test("parseLsMessage keeps an intelligent-match note on a description and a list", () => {
  const note = 'No exact match for "post", so showing similar names.';
  const one = parseLsMessage(`${note}\n\n${detail}`);
  assert.equal(one.note, note);
  assert.equal(one.cache, "");
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
  assert.equal(exact.cache, "");
  assert.equal(exact.description.name, "posts");
  assert.deepEqual(exact.tables, []);

  const cachedDetail = parseLsMessage(`cache: Fetched the table list.\n\n${detail}`);
  assert.equal(cachedDetail.note, "");
  assert.equal(cachedDetail.cache, "Fetched the table list.");
  assert.equal(cachedDetail.description.name, "posts");
  assert.equal(cachedDetail.description.rangeKey.name, "n");
  assert.equal(cachedDetail.description.extra.includes("cache:"), false);

  const cachedList = parseLsMessage(`${note}\ncache: Showing cached table list from 1 min ago.\n\n${summary}`);
  assert.equal(cachedList.note, note);
  assert.equal(cachedList.cache, "Showing cached table list from 1 min ago.");
  assert.equal(cachedList.description, null);
  assert.equal(cachedList.tables.length, 2);
  assert.equal(cachedList.tables[0].name, "posts_v2");

  const cacheFirst = parseLsMessage(`cache: Cache refresh failed: timeout.\n\n${note}\n\n${summary}`);
  assert.equal(cacheFirst.note, note);
  assert.equal(cacheFirst.cache, "Cache refresh failed: timeout.");
  assert.equal(cacheFirst.tables.length, 2);

  const related = parseLsMessage(
    'No exact match for "customer_id", so showing related keys.\n\n' + summary,
  );
  assert.equal(related.note.includes("related keys"), true);
  assert.equal(related.tables.length, 2);

  const mixed = parseLsMessage(
    'No exact match for "order", so showing similar names and related keys.\n\nTables\nName Items\nonly 0',
  );
  assert.equal(mixed.note.includes("similar names and related keys"), true);
  assert.equal(mixed.description, null);
});

test("parseTableSummary reads size text and rejects a description", () => {
  assert.equal(parseTableSummary(detail), null);
  assert.equal(parseTableSummary("Tables\nName Items\nposts 0"), null);
});
