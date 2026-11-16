import assert from "node:assert/strict";
import test from "node:test";

import {
  availableSources,
  attributeLabel,
  buildReadStatement,
  defaultReadState,
  ensureQueryKeys,
  filterAttributes,
  formatCondition,
  quoteIdent,
  quoteValue,
  readError,
  sourceKeys,
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

test("quoteIdent quotes reserved words and odd names", () => {
  assert.equal(quoteIdent("id"), "id");
  assert.equal(quoteIdent("by-status"), "by-status");
  assert.equal(quoteIdent("size"), "'size'");
  assert.equal(quoteIdent("from"), "'from'");
  assert.equal(quoteIdent("odd name"), "'odd name'");
});

test("quoteValue follows the attribute type", () => {
  assert.equal(quoteValue("12", "NUMBER"), "12");
  assert.equal(quoteValue("true", "BOOL"), "TRUE");
  assert.equal(quoteValue("open", "STRING"), "'open'");
  assert.equal(quoteValue("it's", "STRING"), "'it''s'");
  assert.equal(quoteValue("3", ""), "3");
  assert.equal(quoteValue("true", ""), "TRUE");
});

test("scan defaults to the table with no WHERE", () => {
  const sql = buildReadStatement("posts", table, defaultReadState());
  assert.equal(sql, "SCAN * FROM posts");
});

test("query adds a locked hash condition and requires a value", () => {
  const state = ensureQueryKeys(table, { ...defaultReadState(), mode: "query" });
  assert.equal(state.keys[0].attribute, "id");
  assert.equal(state.keys[0].locked, true);
  assert.equal(readError(table, state), "Query needs a hash key value.");
  state.keys[0].value = "ada";
  assert.equal(readError(table, state), "");
  assert.equal(
    buildReadStatement("posts", table, state),
    "SELECT * FROM posts WHERE id = 'ada'",
  );
});

test("key, filter, index, and reverse compose one statement", () => {
  const sql = buildReadStatement("posts", table, {
    source: "by-status",
    mode: "query",
    order: "reverse",
    keys: [
      { attribute: "status", op: "eq", value: "open", role: "hash", locked: true },
      { attribute: "sk", op: "between", value: "1", value2: "8", role: "range" },
    ],
    filters: [{ attribute: "kind", op: "begins_with", value: "ITEM#" }],
  });
  assert.equal(
    sql,
    "SELECT * FROM posts WHERE status = 'open' AND sk BETWEEN 1 AND 8 AND begins_with(kind, 'ITEM#') USING by-status DESC",
  );
});

test("scan reverse orders by the source range key", () => {
  const sql = buildReadStatement("posts", table, {
    source: "",
    mode: "scan",
    order: "reverse",
    keys: [],
    filters: [{ attribute: "status", op: "eq", value: "open" }],
  });
  assert.equal(sql, "SCAN * FROM posts WHERE status = 'open' ORDER BY sk DESC");
});

test("scan omits local indexes; query keeps them", () => {
  const scan = availableSources(table, "scan").map((source) => source.name);
  const query = availableSources(table, "query").map((source) => source.name);
  assert.deepEqual(scan, ["", "by-status"]);
  assert.deepEqual(query, ["", "by-status", "kind-index"]);
  assert.deepEqual(
    sourceKeys(table, "kind-index", "query").map((key) => key.name),
    ["id", "kind"],
  );
});

test("exists and type conditions omit or quote values", () => {
  assert.equal(
    formatCondition({ attribute: "status", op: "exists" }, table),
    "attribute_exists(status)",
  );
  assert.equal(
    formatCondition({ attribute: "status", op: "attribute_type", value: "S" }, table),
    "attribute_type(status, 'S')",
  );
  assert.equal(formatCondition({ attribute: "status", op: "eq", value: "" }, table), "");
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

test("typed attributes that are missing from metadata still format", () => {
  const sql = buildReadStatement("posts", table, {
    ...defaultReadState(),
    filters: [{ attribute: "note", op: "eq", value: "hello" }],
  });
  assert.equal(sql, "SCAN * FROM posts WHERE note = 'hello'");
});

test("query without hash metadata accepts a typed hash name", () => {
  const empty = { name: "posts", keys: [], indexes: [], attributes: [] };
  const state = ensureQueryKeys(empty, { ...defaultReadState(), mode: "query" });
  assert.equal(state.keys[0].attribute, "");
  assert.equal(state.keys[0].locked, false);
  assert.equal(readError(empty, state), "Query needs a hash key.");
  state.keys[0].attribute = "pk";
  assert.equal(readError(empty, state), "Query needs a hash key value.");
  state.keys[0].value = "ada";
  assert.equal(readError(empty, state), "");
  assert.equal(
    buildReadStatement("posts", empty, state),
    "SELECT * FROM posts WHERE pk = 'ada'",
  );
});
