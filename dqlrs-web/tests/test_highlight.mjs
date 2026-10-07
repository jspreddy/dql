import assert from "node:assert/strict";
import test from "node:test";

import { bandAppearance, highlightSource, knownWords, queryBands, runTarget, styleForWord } from "../static/dql-tokens.js";

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
  assert.equal(text.slice(bands[0].from, bands[0].to).startsWith("-- header"), true);
  assert.equal(text.slice(bands[0].from, bands[0].to).includes("'a;b'"), true);
  assert.equal(text.slice(0, bands[0].from), "\n");
  assert.equal(bands[3].write, true);
  assert.equal(bands[4].write, true);
  const tokens = highlightSource("DROP TABLE t; DELETE FROM t;");
  assert.equal(tokens.find((token) => token.text === "DROP").style, "deleted");
  assert.equal(tokens.find((token) => token.text === "DELETE").style, "deleted");
});

test("blank lines bound a highlight and comments stick to the touching query", () => {
  const before = "-- head\nSELECT 1;\n\nSCAN 2;\n";
  const beforeBands = queryBands(before);
  assert.equal(before.slice(beforeBands[0].from, beforeBands[0].to), "-- head\nSELECT 1;\n\n");
  assert.equal(before.slice(beforeBands[1].from, beforeBands[1].to), "SCAN 2;\n");

  const after = "SELECT 1;\n-- tail\n\nSCAN 2;";
  const afterBands = queryBands(after);
  assert.equal(after.slice(afterBands[0].from, afterBands[0].to), "SELECT 1;\n-- tail\n\n");
  assert.equal(after.slice(afterBands[1].from, afterBands[1].to), "SCAN 2;");

  const between = "SELECT 1;\n\n-- next\nDROP TABLE t;";
  const betweenBands = queryBands(between);
  assert.equal(between.slice(betweenBands[0].from, betweenBands[0].to), "SELECT 1;\n\n");
  assert.equal(between.slice(betweenBands[1].from, betweenBands[1].to), "-- next\nDROP TABLE t;");

  const sandwiched = "SELECT 1;\n-- mid\nDELETE FROM t;";
  const sandwichedBands = queryBands(sandwiched);
  assert.equal(sandwiched.slice(sandwichedBands[0].from, sandwichedBands[0].to), "SELECT 1;\n-- mid\n");
  assert.equal(sandwiched.slice(sandwichedBands[1].from, sandwichedBands[1].to), "DELETE FROM t;");

  const leading = "\n\nSELECT 1;\n";
  const leadingBands = queryBands(leading);
  assert.equal(leading.slice(leadingBands[0].from, leadingBands[0].to), "SELECT 1;\n");
  assert.equal(leadingBands[0].lineFrom, 3);
  assert.equal(leadingBands[0].lineTo, 4);
});

test("highlight toggles choose a fill or a gutter bar", () => {
  const writeEven = { write: true, index: 0 };
  const writeOdd = { write: true, index: 1 };
  const readEven = { write: false, index: 2 };
  const readOdd = { write: false, index: 3 };
  const full = { minimalWrite: false, evenOdd: true };
  assert.equal(bandAppearance(writeEven, full), "write-even");
  assert.equal(bandAppearance(writeOdd, full), "write");
  assert.equal(bandAppearance(readEven, full), "alt");
  assert.equal(bandAppearance(readOdd, full), "plain");
  const minimal = { minimalWrite: true, evenOdd: true };
  assert.equal(bandAppearance(writeEven, minimal), "bar-even");
  assert.equal(bandAppearance(writeOdd, minimal), "bar");
  assert.equal(bandAppearance(readEven, minimal), "alt");
  const flat = { minimalWrite: true, evenOdd: false };
  assert.equal(bandAppearance(writeEven, flat), "bar");
  assert.equal(bandAppearance(writeOdd, flat), "bar");
  assert.equal(bandAppearance(readEven, flat), "plain");
  assert.equal(bandAppearance(readOdd, { minimalWrite: false, evenOdd: false }), "plain");
});

test("run target is the selection, or the query at the cursor", () => {
  const src = "-- head\nSELECT 1;\n\nSCAN 2;\n";
  const selectAt = src.indexOf("SELECT");
  const atSelect = runTarget(src, selectAt);
  assert.equal(atSelect.kind, "query");
  assert.equal(src.slice(atSelect.from, atSelect.to), "-- head\nSELECT 1;\n\n");

  const onBlank = runTarget(src, src.indexOf(";\n\n") + 2);
  assert.equal(src.slice(onBlank.from, onBlank.to), "-- head\nSELECT 1;\n\n");

  const atScan = runTarget(src, src.indexOf("SCAN"));
  assert.equal(atScan.kind, "query");
  assert.equal(src.slice(atScan.from, atScan.to), "SCAN 2;\n");
  assert.deepEqual(runTarget(src, src.length), atScan);

  const leading = "\n\nSELECT 1;\n";
  assert.equal(runTarget(leading, 0), null);
  assert.equal(leading.slice(runTarget(leading, leading.indexOf("SELECT")).from), "SELECT 1;\n");

  const sandwiched = "SELECT 1;\n-- mid\nDELETE FROM t;";
  const onMid = runTarget(sandwiched, sandwiched.indexOf("-- mid"));
  assert.equal(sandwiched.slice(onMid.from, onMid.to), "SELECT 1;\n-- mid\n");

  const selected = runTarget(src, src.indexOf("1"), src.indexOf("SCAN") + 4);
  assert.equal(selected.kind, "selection");
  assert.equal(src.slice(selected.from, selected.to), src.slice(src.indexOf("1"), src.indexOf("SCAN") + 4));

  const whitespace = runTarget("SELECT 1;\n", 6, 7);
  assert.equal(whitespace.kind, "query");
  assert.equal("SELECT 1;\n".slice(whitespace.from, whitespace.to), "SELECT 1;\n");
});
