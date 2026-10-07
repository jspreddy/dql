import assert from "node:assert/strict";
import test from "node:test";

import { highlightSource, knownWords, queryBands, styleForWord } from "../static/dql-tokens.js";

const required = {
  keyword: [
    "SELECT", "SCAN", "INSERT", "UPDATE", "CREATE", "ALTER",
    "DUMP", "LOAD", "EXPLAIN", "ANALYZE",
    "FROM", "WHERE", "INTO", "VALUES", "SET", "ADD", "REMOVE", "USING",
    "LIMIT", "ORDER", "BY", "ASC", "DESC", "AS", "KEYS", "IN", "CONSISTENT",
    "THROTTLE", "RETURNS", "SAVE",
    "TABLE", "INDEX", "GLOBAL", "HASH", "RANGE", "KEY", "THROUGHPUT", "TP",
    "IF", "NOT", "EXISTS", "INCLUDE", "ALL", "SCHEMA",
    "AND", "OR", "BETWEEN",
    "NONE", "UPDATED", "OLD", "NEW",
    "LS", "OPT", "OPTIONS", "HELP", "USE", "WATCH",
  ],
  deleted: ["DROP", "DELETE"],
  typeName: ["STRING", "NUMBER", "BINARY", "BOOL", "BOOLEAN"],
  "variableName.function": [
    "COUNT", "SIZE", "BEGINS_WITH", "CONTAINS",
    "ATTRIBUTE_EXISTS", "ATTRIBUTE_NOT_EXISTS", "ATTRIBUTE_TYPE",
    "IF_NOT_EXISTS", "LIST_APPEND",
    "TIMESTAMP", "TS", "UTCTIMESTAMP", "UTCTS", "NOW", "UTCNOW", "MS", "INTERVAL",
  ],
  bool: ["TRUE", "FALSE"],
  null: ["NULL"],
};

test("every DQL keyword has a highlight style", () => {
  const known = new Set(knownWords());
  for (const [style, words] of Object.entries(required)) {
    for (const word of words) {
      assert.equal(styleForWord(word), style, word);
      assert.equal(styleForWord(word.toLowerCase()), style, word);
      assert.ok(known.has(word), word);
    }
  }
});

test("sample script highlights keywords, strings, comments, and numbers", () => {
  const tokens = highlightSource(`
-- keep the string below literal
SELECT count(*) AS n FROM nb_posts
WHERE username = 'SELECT' AND begins_with(body, "he")
  AND score BETWEEN 1 AND 10
  AND flag = true AND deleted = null;
SCAN * FROM t WHERE my-field = -2.5;
CREATE TABLE t (id STRING HASH KEY, THROUGHPUT (1, 1));
UPDATE t SET name = if_not_exists(name, 'x') WHERE id = b'abc';
`);
  const byText = new Map(tokens.map((token) => [token.text, token.style]));
  assert.equal(tokens.find((token) => token.text.startsWith("--")).style, "comment");
  assert.equal(byText.get("SELECT"), "keyword");
  assert.equal(byText.get("count"), "variableName.function");
  assert.equal(byText.get("AS"), "keyword");
  assert.equal(byText.get("FROM"), "keyword");
  assert.equal(byText.get("begins_with"), "variableName.function");
  assert.equal(byText.get("BETWEEN"), "keyword");
  assert.equal(byText.get("true"), "bool");
  assert.equal(byText.get("null"), "null");
  assert.equal(byText.get("STRING"), "typeName");
  assert.equal(byText.get("HASH"), "keyword");
  assert.equal(byText.get("if_not_exists"), "variableName.function");
  assert.equal(byText.get("my-field"), "variableName");
  assert.equal(byText.get("-2.5"), "number");
  assert.equal(byText.get("1"), "number");
  assert.ok(tokens.some((token) => token.text === "'SELECT'" && token.style === "string"));
  assert.ok(tokens.some((token) => token.text === '"he"' && token.style === "string"));
  assert.ok(tokens.some((token) => token.text === "b'abc'" && token.style === "string"));
  assert.equal(tokens.filter((token) => token.text === "SELECT").length, 1);
});

test("queries alternate a light band and writes are marked", () => {
  const text = `
-- header stays outside the first query
SELECT * FROM t WHERE name = 'a;b';
INSERT INTO t (id) VALUES (1);
SCAN * FROM t;
DELETE FROM t WHERE id = 1;
drop table t;
`;
  const bands = queryBands(text);
  assert.deepEqual(
    bands.map((band) => band.band),
    ["alt", "write", "alt", "write", "write-even"],
  );
  assert.equal(text.slice(bands[0].from, bands[0].to).startsWith("SELECT"), true);
  assert.equal(text.slice(bands[0].from, bands[0].to).includes("--"), false);
  assert.equal(text.slice(bands[0].from, bands[0].to).includes("'a;b'"), true);
  assert.equal(bands[3].write, true);
  assert.equal(bands[4].write, true);
  const tokens = highlightSource("DROP TABLE t; DELETE FROM t;");
  assert.equal(tokens.find((token) => token.text === "DROP").style, "deleted");
  assert.equal(tokens.find((token) => token.text === "DELETE").style, "deleted");
});
