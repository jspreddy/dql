import { test, expect } from "../support/app.js";
import { pressMod, replaceEditor } from "../support/helpers.js";

test("opens the table browser from the #tables hash", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/#tables`);
  await expect(page.locator("#tables-view")).toBeVisible();
  await expect(page.locator("#query-view")).toBeHidden();
  await expect(page.locator("#mode-tables")).toHaveAttribute("aria-selected", "true");
  await expect(page.locator("#file-name")).toHaveText("No file open");
  await expect(page.locator("#side-title")).toHaveText("Tables");
  await expect(page.locator("#refresh-tables")).toBeVisible();
  await expect(page.locator("#new-file")).toBeHidden();
  const heading = await page.locator(".side-heading").evaluate((row) =>
    [...row.children].map((child) => child.id),
  );
  expect(heading).toEqual(["side-title", "refresh-tables"]);
  await expect(page.locator("#tree")).toBeHidden();
  await expect(page.locator("#tables-nav")).toBeVisible();
  await expect(page.locator(".side #table-list")).toBeVisible();
  await expect(page.locator("#tables-view #table-list")).toHaveCount(0);

  await page.locator("#mode-query").click();
  await expect(page.locator("#side-title")).toHaveText("Files");
  await expect(page.locator("#refresh-tables")).toBeHidden();
  await expect(page.locator("#tree")).toBeVisible();
  await expect(page.locator("#tables-nav")).toBeHidden();
  await expect(page.locator("#new-file")).toBeVisible();
});

test("refresh button reloads the table list without the description cache", async ({ page, app }) => {
  const first = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return response.ok() && url.pathname === "/api/tables" && url.searchParams.get("refresh") !== "1";
  });
  await page.goto(`${app.baseURL}/#tables`);
  await first;
  const button = page.locator("#refresh-tables");
  await expect(button).toBeVisible();
  await expect(button).toBeEnabled();

  const pending = page.waitForRequest((request) => {
    const url = new URL(request.url());
    return url.pathname === "/api/tables" && url.searchParams.get("refresh") === "1";
  });
  await button.click();
  const request = await pending;
  expect(new URL(request.url()).searchParams.get("pattern")).toBe("");
  await expect(button).toBeEnabled();
  await expect(page.locator("#table-list")).not.toHaveText("Loading…");
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
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "51 affected" })).toBeVisible();

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  const row = page.locator(`#table-${name}`);
  await expect(row).toBeVisible();
  await expect(row).toContainText(`${name}`);
  await expect(row).toContainText("id");
  await expect(row.locator('[data-key="hash"]')).toBeVisible();
  await expect(row).not.toContainText("HASH");
  await expect(row).not.toContainText("RANGE");
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
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th")).toHaveText(["id", "sk", "apple", "n", "zebra"]);
  await expect(page.locator("#result-table thead th").nth(0)).toHaveClass(/col-table/);
  await expect(page.locator("#result-table thead th").nth(0).locator('[data-key="hash"]')).toBeVisible();
  await expect(page.locator("#result-table thead th").nth(0)).toHaveCSS("background-color", "rgb(231, 246, 236)");
  await expect(page.locator("#result-table thead th").nth(0)).toHaveCSS("box-shadow", /rgb\(111, 191, 150\)/);
  const headerIcons = await page.locator("#result-table thead th").nth(0).locator(".col-label").evaluate((label) =>
    [...label.children].map((child) => child.dataset.key || child.textContent.trim()),
  );
  expect(headerIcons).toEqual(["id", "hash"]);
  await expect(page.locator("#result-table thead th").nth(1)).toHaveClass(/col-table/);
  await expect(page.locator("#result-table thead th").nth(1).locator('[data-key="range"]')).toBeVisible();
  await expect(page.locator("#result-table tbody td").nth(0)).toHaveCSS("background-color", "rgb(245, 251, 247)");
  await expect(page.locator("#result-table tbody td").nth(0)).toHaveCSS("box-shadow", /rgb\(111, 191, 150\)/);

  await replaceEditor(page, `SELECT zebra, apple, id FROM ${name} WHERE id = 'a' AND sk = 1;`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th")).toHaveText(["id", "zebra", "apple"]);

  await replaceEditor(page, `SELECT * FROM ${name} WHERE n = 9 AND sk = 2 USING by-n;`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th")).toHaveText(["id", "sk", "n", "apple", "zebra"]);
  await expect(page.locator("#result-table thead th").nth(0)).toHaveClass(/col-table/);
  await expect(page.locator("#result-table thead th").nth(0)).toHaveCSS("background-color", "rgb(231, 246, 236)");
  await expect(page.locator("#result-table thead th").nth(1)).toHaveClass(/col-both/);
  await expect(page.locator("#result-table thead th").nth(1)).toHaveCSS("background-color", "rgb(231, 243, 244)");
  await expect(page.locator("#result-table thead th").nth(1)).toHaveCSS("box-shadow", /rgb\(116, 184, 184\)/);
  await expect(page.locator("#result-table thead th").nth(1).locator('[data-key="range"]')).toBeVisible();
  await expect(page.locator("#result-table thead th").nth(2)).toHaveClass(/col-index/);
  await expect(page.locator("#result-table thead th").nth(2)).toHaveCSS("background-color", "rgb(231, 241, 252)");
  await expect(page.locator("#result-table thead th").nth(2).locator('[data-key="hash"]')).toBeVisible();
  await expect(page.locator("#result-table tbody td").nth(1)).toHaveCSS("background-color", "rgb(245, 250, 251)");
  await expect(page.locator("#result-table thead th").nth(2)).toHaveCSS("box-shadow", /rgb\(122, 166, 224\)/);
  await expect(page.locator("#result-table tbody td").nth(2)).toHaveCSS("background-color", "rgb(245, 249, 254)");

  await page.locator("#mode-tables").click();
  await page.locator("#table-search").fill(name);
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table thead th")).toHaveText(["id", "sk", "apple", "n", "zebra"]);
  await expect(page.locator("#rows-table thead th").nth(0)).toHaveClass(/col-table/);
  await expect(page.locator("#rows-table thead th").nth(0).locator('[data-key="hash"]')).toBeVisible();
  await expect(page.locator("#rows-table thead th").nth(0)).toHaveCSS("background-color", "rgb(231, 246, 236)");
  await expect(page.locator("#rows-table thead th").nth(1).locator('[data-key="range"]')).toBeVisible();
  await expect(page.locator("#rows-table thead th").nth(3)).not.toHaveClass(/col-/);
  const listed = page.locator(`#table-${name}`);
  await expect(listed.locator('[data-key="hash"]')).toBeVisible();
  await expect(listed.locator('[data-key="range"]')).toBeVisible();
  await expect(listed).toContainText("id");
  await expect(listed).toContainText("sk");
  await expect(listed).not.toContainText("HASH");
  await expect(listed).not.toContainText("RANGE");
  const listedOrder = await listed.locator(".key-pair").evaluateAll((pairs) =>
    pairs.map((pair) => [...pair.children].map((child) => child.dataset.key || child.textContent.trim())),
  );
  expect(listedOrder).toEqual([["hash", "id"], ["range", "sk"]]);
});

test("tables-search-matches-like-ls", async ({ page, app }) => {
  const stem = uniqueTable();
  const catalog = `${stem}catalog`;
  const editions = `${stem}editions`;
  const orders = `red${uniqueTable()}`;
  const shipments = `blue${uniqueTable()}`;
  const key = `zzqq${Math.random().toString(36).slice(2, 8)}`;
  for (const table of [catalog, editions, orders, shipments]) app.trackTable(table);

  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  await replaceEditor(
    page,
    [
      `CREATE TABLE ${catalog} (isbn STRING HASH KEY, title STRING);`,
      `CREATE TABLE ${editions} (isbn STRING HASH KEY, edition NUMBER RANGE KEY);`,
      `CREATE TABLE ${orders} (${key} STRING HASH KEY, title STRING);`,
      `CREATE TABLE ${shipments} (${key} STRING HASH KEY, shipment_id STRING RANGE KEY);`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="run-status-ok"]')).toHaveCount(4);
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);

  await page.locator("#mode-tables").click();
  await expect(page.locator("#table-list")).not.toHaveText("Loading…");

  await page.locator("#table-search").fill(stem);
  const similar = page.locator("#table-list [data-testid='intelligent-match']");
  await expect(similar).toHaveText("Similar names");
  await expect(similar).toHaveAttribute("title", `No exact match for "${stem}", so showing similar names.`);
  await expect(page.locator(`#table-${catalog}`)).toBeVisible();
  await expect(page.locator(`#table-${editions}`)).toBeVisible();
  await expect(page.locator(`#table-${catalog}`)).toContainText("isbn");

  await page.locator("#table-search").fill(`${stem}*`);
  await expect(page.locator(`#table-${catalog}`)).toBeVisible();
  await expect(page.locator(`#table-${editions}`)).toBeVisible();
  await expect(page.locator("#table-list [data-testid='intelligent-match']")).toHaveCount(0);

  await page.locator("#table-search").fill(catalog);
  await expect(page.locator(`#table-${catalog}`)).toBeVisible();
  await expect(page.locator(`#table-${editions}`)).toHaveCount(0);
  await expect(page.locator("#table-list [data-testid='intelligent-match']")).toHaveCount(0);

  await page.locator("#table-search").fill(key);
  const related = page.locator("#table-list [data-testid='intelligent-match']");
  await expect(related).toHaveText("Related keys");
  await expect(related).toHaveAttribute("title", `No exact match for "${key}", so showing related keys.`);
  await expect(page.locator(`#table-${orders}`)).toBeVisible();
  await expect(page.locator(`#table-${shipments}`)).toBeVisible();
  await expect(page.locator(`#table-${shipments}`)).toContainText(key);
  await expect(page.locator(`#table-${catalog}`)).toHaveCount(0);

  await page.locator("#table-search").fill("zzz_no_such_table_*");
  await expect(page.locator("#table-list")).toContainText("No tables match zzz_no_such_table_*");
  await expect(page.locator("#table-list [data-testid='intelligent-match']")).toHaveCount(0);
});

function uniqueTable() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}
