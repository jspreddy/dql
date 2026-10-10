import { test, expect } from "../support/app.js";
import { replaceEditor } from "../support/helpers.js";

test("opens the table browser from the #tables hash", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/#tables`);
  await expect(page.locator("#tables-view")).toBeVisible();
  await expect(page.locator("#query-view")).toBeHidden();
  await expect(page.locator("#mode-tables")).toHaveAttribute("aria-selected", "true");
  await expect(page.locator("#file-name")).toHaveText("No file open");
  await expect(page.locator("#side-title")).toHaveText("Tables");
  await expect(page.locator("#tree")).toBeHidden();
  await expect(page.locator("#tables-nav")).toBeVisible();
  await expect(page.locator(".side #table-list")).toBeVisible();
  await expect(page.locator("#tables-view #table-list")).toHaveCount(0);

  await page.locator("#mode-query").click();
  await expect(page.locator("#side-title")).toHaveText("Files");
  await expect(page.locator("#tree")).toBeVisible();
  await expect(page.locator("#tables-nav")).toBeHidden();
  await expect(page.locator("#new-file")).toBeVisible();
});

test("pages a table fifty rows at a time from a searched name", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  const tuples = Array.from({ length: 51 }, (_, index) => `('r${String(index).padStart(2, "0")}', ${index})`).join(", ");
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY);`,
      `INSERT INTO ${name} (id, n) VALUES ${tuples};`,
    ].join("\n"),
  );
  await page.keyboard.press("Control+A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "51 affected" })).toBeVisible();

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  const row = page.locator(`#table-${name}`);
  await expect(row).toBeVisible();
  await expect(row).toContainText(`${name}`);
  await expect(row).toContainText("id HASH");
  await expect(page.locator("#rows-title")).toHaveText(name);
  await expect(page.locator("#rows-meta")).toHaveText("50 rows at a time");
  await expect(page.locator("#page-label")).toHaveText("Showing 1–50");
  await expect(page.locator("#page-prev")).toBeDisabled();
  await expect(page.locator("#page-next")).toBeEnabled();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(50);

  await page.locator("#table-search").fill("zzz_no_such_table_*");
  await expect(page.locator("#table-list")).toContainText("No tables match zzz_no_such_table_*");

  await page.locator("#table-search").fill(name);
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#page-label")).toHaveText("Showing 1–50");
  await page.locator("#page-next").click();
  await expect(page.locator("#page-label")).toHaveText("Showing 51–51");
  await expect(page.locator("#page-next")).toBeDisabled();
  await expect(page.locator("#page-prev")).toBeEnabled();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(1);

  await page.locator("#page-prev").click();
  await expect(page.locator("#page-label")).toHaveText("Showing 1–50");
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(50);
});

test("orders table and result columns by keys, index, then selection or name", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY, sk NUMBER RANGE KEY) GLOBAL INDEX ('by-n', n NUMBER, sk NUMBER);`,
      `INSERT INTO ${name} (id, sk, zebra, n, apple) VALUES ('a', 2, 'z', 9, 'p'), ('a', 1, 'y', 3, 'q');`,
      `SELECT * FROM ${name} WHERE id = 'a';`,
    ].join("\n"),
  );
  await page.keyboard.press("Control+A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th")).toHaveText(["id", "sk", "apple", "n", "zebra"]);

  await replaceEditor(page, `SELECT zebra, apple, id FROM ${name} WHERE id = 'a' AND sk = 1;`);
  await page.keyboard.press("Control+A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th")).toHaveText(["id", "zebra", "apple"]);

  await replaceEditor(page, `SELECT * FROM ${name} WHERE n = 9 AND sk = 2 USING by-n;`);
  await page.keyboard.press("Control+A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th")).toHaveText(["id", "sk", "n", "apple", "zebra"]);

  await page.locator("#mode-tables").click();
  await page.locator("#table-search").fill(name);
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table thead th")).toHaveText(["id", "sk", "apple", "n", "zebra"]);
});

function uniqueTable() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}
