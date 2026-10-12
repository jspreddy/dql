import { test, expect } from "../support/app.js";
import { node, openApp, pressMod, replaceEditor } from "../support/helpers.js";

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

async function runEditor(page, text) {
  await replaceEditor(page, text);
  await expect(page.locator("#editor-content")).toContainText(text.split("\n")[0]);
  await pressMod(page, "A");
  await page.locator("#run").click();
}

function uniqueTable() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

function seed(table) {
  return [
    `DROP TABLE IF EXISTS ${table};`,
    `CREATE TABLE ${table} (id STRING HASH KEY, sk STRING RANGE KEY);`,
    `INSERT INTO ${table} (id='store-1', sk='ITEM#mug', name='hello world', status='active', title='Blue mug', tags=('lighting', 'ceramic'), items=['handle', 'glaze'], qty=3);`,
    `INSERT INTO ${table} (id='store-1', sk='ITEM#lamp', name='goodbye', status='draft', title='Arc lamp', tags=('lighting', 'metal', 'floor'), items=['shade', 'cord'], qty=1);`,
    `INSERT INTO ${table} (id='store-1', sk='NOTE#1', name='memo', title='Field notes');`,
    `INSERT INTO ${table} (id='store-2', sk='ITEM#cable', name='usb world', status='active', title='USB-C cable', tags=('cable'), items=[], qty=12);`,
  ];
}

test("opens the functions sample and runs every condition function", async ({ page, app }) => {
  app.trackTable("sample_functions");
  await node(page, "queries/functions.dql").click();
  await expect(page.locator("#file-name")).toHaveText("functions.dql");
  await expect(page.locator("#editor-content")).toContainText("contains");
  await expect(page.locator("#editor-content")).toContainText("begins_with");
  await expect(page.locator("#editor-content")).toContainText("attribute_exists");
  await expect(page.locator("#editor-content")).toContainText("attribute_not_exists");
  await expect(page.locator("#editor-content")).toContainText("attribute_type");
  await expect(page.locator("#editor-content")).toContainText("size(tags)");

  await page.locator("#editor-content").click();
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator(".result-card")).toHaveCount(12);
  await expect(page.locator("#result-meta")).toHaveText("12 results");
  const results = page.locator("#result-body");
  await expect(results).toContainText("Blue mug");
  await expect(results).toContainText("Arc lamp");
  await expect(results).toContainText("Field notes");
  await expect(results).toContainText("USB-C cable");
  await expect(page.locator("#result-table")).toContainText("Field notes");
  await expect(page.locator("#result-table")).toContainText("USB-C cable");
  await expect(page.locator('[data-testid="explain-plan"]')).toBeVisible();
  await expect(page.locator('[data-testid="explain-plan"]')).toContainText("contains");
});

test("returns the expected rows for each DynamoDB function", async ({ page, app }) => {
  const table = uniqueTable();
  app.trackTable(table);
  await runEditor(page, seed(table).join("\n"));
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "1 affected" })).toHaveCount(4);

  const cases = [
    {
      dql: `SELECT * FROM ${table} WHERE id = 'store-1' AND begins_with(sk, 'ITEM#')`,
      rows: 2,
      text: ["Blue mug", "Arc lamp"],
      missing: ["Field notes"],
    },
    {
      dql: `SELECT title FROM ${table} WHERE id = 'store-1' AND contains(name, 'hello')`,
      rows: 1,
      text: ["Blue mug"],
    },
    {
      dql: `SCAN * FROM ${table} WHERE contains('name', 'hello')`,
      rows: 1,
      text: ["hello world"],
    },
    {
      dql: `SCAN title FROM ${table} WHERE contains(status, 'act')`,
      rows: 2,
      text: ["Blue mug", "USB-C cable"],
    },
    {
      dql: `SCAN * FROM ${table} WHERE contains(tags, 'metal')`,
      rows: 1,
      text: ["Arc lamp"],
    },
    {
      dql: `SCAN * FROM ${table} WHERE contains(items, 'cord')`,
      rows: 1,
      text: ["Arc lamp"],
    },
    {
      dql: `SCAN * FROM ${table} WHERE begins_with(title, 'Blue')`,
      rows: 1,
      text: ["Blue mug"],
    },
    {
      dql: `SCAN * FROM ${table} WHERE attribute_exists(tags)`,
      rows: 3,
      text: ["Blue mug", "Arc lamp", "USB-C cable"],
      missing: ["Field notes"],
    },
    {
      dql: `SCAN * FROM ${table} WHERE attribute_not_exists(tags)`,
      rows: 1,
      text: ["Field notes"],
    },
    {
      dql: `SCAN * FROM ${table} WHERE attribute_type(qty, N)`,
      rows: 3,
      text: ["Blue mug", "Arc lamp", "USB-C cable"],
      missing: ["Field notes"],
    },
    {
      dql: `SCAN * FROM ${table} WHERE size(tags) > 2`,
      rows: 1,
      text: ["Arc lamp"],
    },
    {
      dql: `SCAN * FROM ${table} WHERE size(title) > 8`,
      rows: 2,
      text: ["Field notes", "USB-C cable"],
    },
  ];

  for (const { dql, rows, text, missing } of cases) {
    await runEditor(page, dql);
    const errors = page.locator('[data-testid="result-note"].error');
    if ((await errors.count()) > 0) {
      throw new Error(`unexpected error for ${dql}: ${await errors.first().innerText()}`);
    }
    await expect(page.locator("#result-meta"), dql).toHaveText(`${rows} ${rows === 1 ? "row" : "rows"}`);
    for (const value of text) {
      await expect(page.locator("#result-table"), dql).toContainText(value);
    }
    for (const value of missing || []) {
      await expect(page.locator("#result-table"), dql).not.toContainText(value);
    }
  }
});
