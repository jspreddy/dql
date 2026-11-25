// The table browser's picker helpers. Statements are built on the server (web/table_read.rs);
// its golden cases are rust-impl/crates/dql-cli/tests/fixtures/table_reads.json.
import assert from "node:assert/strict";
import test from "node:test";

import {
  TABLE_SOURCE,
  availableSources,
  attributeLabel,
  batchKeyHint,
  defaultReadState,
  ensureQueryKeys,
  filterAttributes,
  keyHighlightSpans,
  opsFor,
  readRequest,
  sourceKeys,
  sourceOf,
} from "../static/table-query.js";

const table = {
  name: "posts",
  keys: [
    { name: "id", role: "hash" },
    { name: "sk", role: "range" },
  ],
  indexes: [
    { name: "by-status", kind: "global", hashKey: "status", rangeKey: "sk" },
    { name: "kind-index", kind: "local", hashKey: "id", rangeKey: "kind" },
  ],
  attributes: [
    { name: "id", type: "STRING" },
    { name: "sk", type: "NUMBER" },
    { name: "status", type: "STRING" },
    { name: "kind", type: "STRING" },
  ],
};

test("query adds a locked hash row for the source's hash key", () => {
  const state = ensureQueryKeys(table, { ...defaultReadState(), mode: "query" });
  assert.equal(state.keys[0].attribute, "id");
  assert.equal(state.keys[0].locked, true);
  const indexed = ensureQueryKeys(table, { ...defaultReadState(), mode: "query", source: "by-status" });
  assert.equal(indexed.keys[0].attribute, "status");
  const empty = ensureQueryKeys({ name: "bare", keys: [], indexes: [], attributes: [] }, { ...defaultReadState(), mode: "query" });
  assert.equal(empty.keys[0].attribute, "");
  assert.equal(empty.keys[0].locked, false);
});

test("scan omits local indexes; query keeps them", () => {
  const scan = availableSources(table, "scan").map((source) => source.name);
  const query = availableSources(table, "query").map((source) => source.name);
  assert.deepEqual(scan, ["", TABLE_SOURCE, "by-status"]);
  assert.deepEqual(query, ["", TABLE_SOURCE, "by-status", "kind-index"]);
  assert.deepEqual(
    sourceKeys(table, "kind-index", "query").map((key) => key.name),
    ["id", "kind"],
  );
  assert.equal(sourceOf(table, "missing", "scan").name, "");
});

test("key rows offer only the ops their role allows", () => {
  assert.deepEqual(opsFor("key", "hash", "query").map((op) => op.id), ["eq"]);
  assert.ok(!opsFor("key", "range", "query").some((op) => op.id === "ne"));
  assert.ok(opsFor("key", "range", "query").some((op) => op.id === "begins_with"));
  assert.ok(opsFor("filter", "", "scan").some((op) => op.id === "size"));
});

test("filterAttributes lists keys, indexes, schema, then extras", () => {
  const extra = new Map([["note", "STRING"], ["id", "STRING"]]);
  assert.deepEqual(
    filterAttributes(table, extra).map((item) => item.name),
    ["id", "sk", "status", "kind", "note"],
  );
  assert.equal(attributeLabel({ name: "id", role: "hash" }), "id (hash)");
  assert.equal(attributeLabel({ name: "note" }), "note");
});

test("the Keys hint names the source's keys", () => {
  assert.equal(batchKeyHint(table), "id keys, or (id, sk) pairs");
  assert.equal(batchKeyHint({ ...table, keys: [{ name: "id", role: "hash" }] }), "id keys: a, b, c");
});

test("readRequest is the read the server builds", () => {
  const read = readRequest(
    table,
    {
      ...defaultReadState(),
      mode: "query",
      source: "by-status",
      order: "reverse",
      keys: [{ id: "x", attribute: "status", op: "eq", value: "open", locked: true }],
      filters: [{ id: "y", attribute: "kind", op: "size", value: 2, sizeOp: "le" }],
    },
    new Map([["note", ""]]),
  );
  assert.deepEqual(read, {
    table: "posts",
    mode: "query",
    source: "by-status",
    order: "reverse",
    keys: [{ attribute: "status", op: "eq", value: "open", value2: "", role: "hash", sizeOp: "" }],
    filters: [{ attribute: "kind", op: "size", value: "2", value2: "", role: "", sizeOp: "le" }],
    batchKeys: "",
    extra: ["note"],
  });
  // A read with no rows is a forward Scan of the table.
  assert.deepEqual(readRequest(table, defaultReadState(), new Map()), {
    table: "posts",
    mode: "scan",
    source: "",
    order: "forward",
    keys: [],
    filters: [],
    batchKeys: "",
    extra: [],
  });
});

test("keyHighlightSpans marks keys, tuple parts, punctuation, and errors", () => {
  const marks = (text) => {
    const spans = keyHighlightSpans(text);
    assert.equal(spans.map((span) => span.text).join(""), text, "covers every character");
    return spans.filter((span) => span.kind).map((span) => `${span.text}:${span.kind}${span.key === null ? "" : "#" + span.key}`);
  };
  assert.deepEqual(marks("a, b"), ["a:hash#0", ",:punct", "b:hash#1"]);
  assert.deepEqual(marks("(a, 1), c"), ["(:tuple#0", "a:hash#0", ",:punct#0", "1:range#0", "):tuple#0", ",:punct", "c:hash#1"]);
  assert.deepEqual(marks("((a, 1))"), ["(:punct", "(:tuple#0", "a:hash#0", ",:punct#0", "1:range#0", "):tuple#0", "):punct"]);
  assert.deepEqual(marks("new york\n(b, 2)"), ["new york:hash#0", "(:tuple#1", "b:hash#1", ",:punct#1", "2:range#1", "):tuple#1"]);
  assert.deepEqual(marks("'x, y', z"), ["'x, y':hash#0", ",:punct", "z:hash#1"]);
  assert.ok(marks("(a, b, c)").every((mark) => mark.includes(":error") || mark.includes(":punct")));
  assert.deepEqual(marks("'open"), ["'open:error#0"]);
  assert.deepEqual(marks("a)"), ["a:hash#0", "):error"]);
  assert.deepEqual(marks(""), []);
});
