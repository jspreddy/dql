import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { pressMod, replaceEditor } from "../support/helpers.js";

// The key format in the query editor (KEYS IN and WHERE ... IN) and in the table viewer's
// Keys and Query modes: a list is hash keys, a tuple is a (hash, range) pair.

test.use({ viewport: { width: 1280, height: 760 } });

function uniqueTable(kind) {
  return `keys_demo_${kind}_${Date.now() % 1e7}`;
}

async function runInEditor(page, statement) {
  await replaceEditor(page, statement);
  await pressMod(page, "A");
  await page.locator(".result-card, [data-testid='result-note']").evaluateAll((nodes) => nodes.forEach((node) => node.setAttribute("data-stale", "")));
  await page.locator("#run").click();
  await expect(page.locator("#run")).toBeEnabled();
}

test("key format: query editor and table viewer", async ({ page, app }, testInfo) => {
  const shot = (name) => page.screenshot({ path: playwrightArtifact(`keys-format-${name}-${testInfo.project.name}.png`) });
  const table = uniqueTable("hr");
  app.trackTable(table);
  app.command(`CREATE TABLE ${table} (id STRING HASH KEY, sk NUMBER RANGE KEY);`);
  app.command(`INSERT INTO ${table} (id, sk, tag) VALUES ('a', 1, 'a-one'), ('a', 2, 'a-two'), ('b', 1, 'b-one'), ('c', 3, 'c-three'), ('d', 4, 'd-four');`);

  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  // Query editor.
  const editorCases = [
    ["01-editor-keys-in-hash-list", `SELECT * FROM ${table} KEYS IN a, b;`, 3],
    ["02-editor-keys-in-pairs", `SELECT * FROM ${table} KEYS IN (a, 1), (b, 1), (c, 3);`, 3],
    ["03-editor-keys-in-mixed", `SELECT * FROM ${table} KEYS IN (a, 1), c;`, 2],
    ["04-editor-where-hash-in", `SELECT * FROM ${table} WHERE id IN (a, b);`, 3],
    ["05-editor-where-pairs-in", `SELECT * FROM ${table} WHERE (id, sk) IN ((a, 2), (d, 4));`, 2],
    ["06-editor-where-pairs-filter", `SCAN * FROM ${table} WHERE (id, sk) IN ((a, 1), (a, 2), (b, 1)) AND tag <> 'a-two';`, 2],
  ];
  for (const [name, statement, rows] of editorCases) {
    await runInEditor(page, statement);
    await expect(page.locator(".result-card:not([data-stale]) table tbody tr")).toHaveCount(rows);
    await shot(name);
  }
  await runInEditor(page, `SELECT * FROM ${table} KEYS IN (a, b, c);`);
  await expect(page.locator("#result-body")).toContainText("at most two parts");
  await shot("07-editor-keys-in-three-part-error");
  await runInEditor(page, `EXPLAIN SELECT * FROM ${table} WHERE (id, sk) IN ((a, 2), (d, 4));`);
  await expect(page.locator("#result-body")).toContainText("batch_get_item");
  await shot("08-editor-explain-pairs-batch-get");

  // Table viewer.
  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, table);
  await page.locator("#mode-tables").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(5);
  const keys = page.locator('[data-testid="table-query-batch-keys"]');
  const run = page.locator("#table-query-run");
  await page.locator("#table-query-keys-mode").click();
  for (const [name, input, rows] of [
    ["09-viewer-keys-hash-list", "a, b", 3],
    ["10-viewer-keys-pairs", "(a, 1), (b, 1), (c, 3)", 3],
    ["11-viewer-keys-mixed", "(a, 1), c", 2],
  ]) {
    await keys.fill(input);
    await expect(page.locator("#table-query-error")).toHaveText("");
    await run.click();
    await expect(page.locator("#rows-table tbody tr")).toHaveCount(rows);
    await shot(name);
  }
  await keys.fill("(a, b, c)");
  await expect(page.locator("#table-query-error")).toContainText("at most two parts");
  await shot("12-viewer-keys-three-part-error");
  await keys.fill("(a, x)");
  await expect(page.locator("#table-query-error")).toHaveText('sk is a number. "x" is not.');
  await shot("13-viewer-keys-number-error");

  // Query mode: the WHERE the viewer builds from key conditions.
  await page.locator("#table-query-query").click();
  await page.locator('[data-testid="table-query-key-row"] [data-testid="table-query-value"]').first().fill("a");
  await expect(page.locator("#table-query-error")).toHaveText("");
  await run.click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(2);
  await shot("14-viewer-query-where-hash");
});
