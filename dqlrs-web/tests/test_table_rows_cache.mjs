import assert from "node:assert/strict";
import test from "node:test";

import { createRowsCache } from "../static/table-rows-cache.js";

function page(n) {
  return { page: n, items: [{ id: n }] };
}

test("stores pages for one table and DQL", () => {
  const cache = createRowsCache();
  cache.beginRun("nums", "SELECT * FROM nums WHERE gid = 'g'");
  cache.set("nums", "SELECT * FROM nums WHERE gid = 'g'", 0, page(0));
  cache.set("nums", "SELECT * FROM nums WHERE gid = 'g'", 1, page(1));
  assert.equal(cache.size(), 2);
  assert.deepEqual(cache.get("nums", "SELECT * FROM nums WHERE gid = 'g'", 0).page, 0);
  assert.deepEqual(cache.get("nums", "SELECT * FROM nums WHERE gid = 'g'", 1).page, 1);
  assert.equal(cache.get("nums", "SELECT * FROM nums WHERE gid = 'g'", 2), null);
});

test("beginRun drops pages even when table and DQL are unchanged", () => {
  const cache = createRowsCache();
  cache.beginRun("nums", "SELECT * FROM nums");
  cache.set("nums", "SELECT * FROM nums", 0, page(0));
  const first = cache.generation;
  cache.beginRun("nums", "SELECT * FROM nums");
  assert.notEqual(cache.generation, first);
  assert.equal(cache.size(), 0);
  assert.equal(cache.get("nums", "SELECT * FROM nums", 0), null);
});

test("a different table or DQL does not reuse pages", () => {
  const cache = createRowsCache();
  cache.beginRun("nums", "SELECT * FROM nums");
  cache.set("nums", "SELECT * FROM nums", 0, page(0));
  assert.equal(cache.get("other", "SELECT * FROM nums", 0), null);
  assert.equal(cache.get("nums", "SCAN * FROM nums", 0), null);
  cache.set("nums", "SCAN * FROM nums", 0, page(9));
  assert.equal(cache.size(), 1);
  assert.equal(cache.get("nums", "SELECT * FROM nums", 0), null);
  assert.equal(cache.get("nums", "SCAN * FROM nums", 0).page, 9);
});

test("evicts the least recently used page once the bound is reached", () => {
  const cache = createRowsCache({ maxPages: 3 });
  cache.beginRun("nums", "");
  cache.set("nums", "", 0, page(0));
  cache.set("nums", "", 1, page(1));
  cache.set("nums", "", 2, page(2));
  cache.get("nums", "", 0);
  cache.set("nums", "", 3, page(3));
  assert.equal(cache.size(), 3);
  assert.equal(cache.get("nums", "", 1), null);
  assert.equal(cache.get("nums", "", 0).page, 0);
  assert.equal(cache.get("nums", "", 2).page, 2);
  assert.equal(cache.get("nums", "", 3).page, 3);
});
