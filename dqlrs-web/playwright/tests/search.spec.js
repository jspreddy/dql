import { test, expect } from "../support/app.js";
import { pressMod, replaceEditor } from "../support/helpers.js";

test("title search sits on the title bar and opens a matching query", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const searchBox = await page.locator("#title-search").boundingBox();
  const modes = await page.locator(".modes").boundingBox();
  const settings = await page.locator("#settings-toggle").boundingBox();
  expect(searchBox.x).toBeGreaterThan(modes.x + modes.width - 1);
  expect(searchBox.x + searchBox.width).toBeLessThanOrEqual(settings.x + 1);

  const search = page.locator("#title-search");
  await search.fill("pw_missing_table");
  const hit = page.locator(".title-search-item").filter({ hasText: "SELECT * FROM pw_missing_table" });
  await expect(page.locator(".title-search-group", { hasText: "Queries" })).toBeVisible();
  await expect(hit).toBeVisible();
  await expect(hit).toContainText("Query");
  await expect(hit).toContainText("queries/read.dql");
  await hit.click();

  await expect(page.locator("#title-search-list")).toBeHidden();
  await expect(page.locator("#file-name")).toHaveText("read.dql");
  await expect(page.locator("#mode-query")).toHaveAttribute("aria-selected", "true");
  await expect(page.locator(".cm-activeLine")).toContainText("SELECT * FROM pw_missing_table");
});

test("title search opens a table from a cached column or index", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  const column = `shelf${name}`;
  const index = `by${name}`;
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY, sk STRING RANGE KEY) GLOBAL INDEX ('${index}', ${column} STRING, id STRING);`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="run-status-ok"]')).toHaveCount(2);

  const search = page.locator("#title-search");
  await search.fill(column);
  const columnHit = page.locator(".title-search-item").filter({ hasText: column });
  await expect(columnHit).toBeVisible();
  await expect(columnHit).toContainText("Column");
  await expect(columnHit).toContainText(name);
  await expect(columnHit).toContainText(index);
  await columnHit.click();
  await expect(page.locator("#mode-tables")).toHaveAttribute("aria-selected", "true");
  await expect(page.locator("#rows-title")).toHaveText(name);
  await expect(page.locator(`#table-${name}`)).toHaveClass(/selected/);

  await search.fill(index);
  const indexHit = page.locator(".title-search-item").filter({ hasText: index });
  await expect(indexHit).toContainText("Index");
  await expect(indexHit).toContainText(column);

  await search.fill(name);
  await expect(page.locator(".title-search-item").filter({ hasText: name }).filter({ hasText: "Table" })).toBeVisible();
});

test("title search does not show columns for a table that was only listed", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/#tables`);
  await expect(page.locator("#table-list .table-row").first()).toBeVisible();
  const listed = (await page.locator("#table-list .table-row strong").first().innerText()).trim();
  const search = page.locator("#title-search");
  await search.fill(listed);
  await expect(page.locator(".title-search-item").filter({ hasText: "Table" }).filter({ hasText: listed })).toBeVisible();
  await expect(page.locator(".title-search-item").filter({ hasText: "Column" })).toHaveCount(0);

  await search.fill(`zz_no_such_column_${app.workspace.length}`);
  await expect(page.locator(".title-search-empty")).toHaveText("No matches");
});

function uniqueTable() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}
