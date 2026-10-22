import { test, expect } from "../support/app.js";
import { colorPattern, openResultPane, pressMod, replaceEditor, themeColor } from "../support/helpers.js";

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
  expect(heading).toEqual(["side-title", "refresh-tables", "table-cache"]);
  await expect(page.locator("#tree")).toBeHidden();
  await expect(page.locator("#tables-nav")).toBeVisible();
  await expect(page.locator(".side #table-list")).toBeVisible();
  await expect(page.locator("#tables-view #table-list")).toHaveCount(0);

  await page.locator("#mode-query").click();
  await expect(page.locator("#side-title")).toHaveText("Files");
  await expect(page.locator("#refresh-tables")).toBeHidden();
  await expect(page.locator("#table-cache")).toBeHidden();
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
  const params = new URL(request.url()).searchParams;
  expect(params.get("pattern")).toBe("");
  expect(params.get("async")).toBe("1");
  await expect(button).toBeEnabled();
  await expect(page.locator("#table-list")).not.toHaveText("Loading…");
  const cache = page.locator("#table-cache");
  await expect(cache).toBeVisible();
  await expect(cache.locator(".table-cache-age")).toHaveText(/^\((just now|\d+s ago|\d+ min ago|\d+ hr ago|\d+ day ago|\d+ days ago)\)$/);
  await cache.hover();
  await expect(cache.locator(".table-cache-tip")).toBeVisible();
  await expect(cache.locator(".table-cache-tip")).toContainText("cached");
});

test("settings menu clears the table cache", async ({ page, app }) => {
  const first = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return response.ok() && url.pathname === "/api/tables" && url.searchParams.get("refresh") !== "1";
  });
  await page.goto(`${app.baseURL}/#tables`);
  await first;
  await expect(page.locator("#table-cache")).toBeVisible();

  const settings = await page.locator("#settings-toggle").boundingBox();
  const theme = await page.locator("#theme-toggle").boundingBox();
  expect(settings.x + settings.width).toBeLessThanOrEqual(theme.x + 1);
  expect(Math.abs(settings.y - theme.y)).toBeLessThan(4);

  await page.locator("#settings-toggle").click();
  await expect(page.locator("#settings-pop")).toBeVisible();
  await expect(page.locator("#opt-minimal-write")).toBeVisible();
  await expect(page.locator("#opt-even-odd")).toBeVisible();
  const sidebarAge = await page.locator(".table-cache-age").innerText();
  await expect(page.locator("#settings-cache-age")).toHaveText(sidebarAge);
  await expect(page.locator("#settings-cache-age")).toHaveAttribute("title", /.+/);

  const cleared = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return response.request().method() === "POST" && url.pathname === "/api/table-cache/clear" && response.ok();
  });
  const listed = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return response.ok() && url.pathname === "/api/tables";
  });
  await page.locator("#clear-table-cache").click();
  await cleared;
  await listed;
  await expect(page.locator("#cache-clear-note")).toHaveText("Table cache cleared.");
  await expect(page.locator("#clear-table-cache")).toBeEnabled();
  await expect(page.locator(".table-cache-tip")).toHaveText("Fetched the table list.");
  await expect(page.locator(".table-cache-age")).toHaveText("(just now)");
  await expect(page.locator("#settings-cache-age")).toHaveText("(just now)");
});

test("settings menu shows the cache age before the table list is open", async ({ page, app }) => {
  await page.goto(app.baseURL);
  await page.locator("#settings-toggle").click();
  await expect(page.locator("#settings-pop")).toBeVisible();
  await expect(page.locator("#settings-cache-age")).toHaveText("not cached");
  await expect(page.locator("#settings-cache-age")).toHaveAttribute("title", "No table list cached.");
});

test("searching tables filters the cached list without refresh", async ({ page, app }) => {
  const first = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return response.ok() && url.pathname === "/api/tables" && url.searchParams.get("refresh") !== "1";
  });
  await page.goto(`${app.baseURL}/#tables`);
  await first;
  await expect(page.locator("#table-list")).not.toHaveText("Loading…");

  const tablesRequests = [];
  page.on("request", (request) => {
    const url = new URL(request.url());
    if (url.pathname === "/api/tables") tablesRequests.push(url);
  });

  await page.locator("#table-search").fill("zzz_no_such_table_*");
  await expect(page.locator("#table-list")).toContainText("No tables match zzz_no_such_table_*");
  await expect(page.locator("#table-list")).not.toHaveText("Loading…");
  expect(tablesRequests.length).toBeGreaterThan(0);
  expect(tablesRequests.every((url) => url.searchParams.get("refresh") !== "1")).toBe(true);
  expect(tablesRequests.some((url) => url.searchParams.get("pattern") === "zzz_no_such_table_*")).toBe(true);

  const pending = page.waitForRequest((request) => {
    const url = new URL(request.url());
    return url.pathname === "/api/tables" && url.searchParams.get("refresh") === "1";
  });
  await page.locator("#refresh-tables").click();
  const request = await pending;
  expect(new URL(request.url()).searchParams.get("pattern")).toBe("zzz_no_such_table_*");
  await expect(page.locator("#table-list")).toContainText("No tables match zzz_no_such_table_*");
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
      `CREATE TABLE ${name} (id STRING HASH KEY, sk NUMBER RANGE KEY) GLOBAL INDEX ('by-n', n NUMBER, sk NUMBER) GLOBAL INDEX ('by-apple', apple STRING, id STRING);`,
      `INSERT INTO ${name} (id, sk, zebra, n, apple) VALUES ('a', 2, 'z', 9, 'p'), ('a', 1, 'y', 3, 'q');`,
      `SELECT * FROM ${name} WHERE id = 'a';`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th")).toHaveText(["id", "sk", "apple", "n", "zebra"]);
  await expect(page.locator("#result-table thead th").nth(0)).toHaveClass(/col-table/);
  await expect(page.locator("#result-table thead th").nth(0).locator('[data-key="hash"]')).toBeVisible();
  const tableHead = await themeColor(page, "--col-table-head");
  const tableLine = colorPattern(await themeColor(page, "--col-table-line"));
  const tableCell = await themeColor(page, "--col-table-cell");
  const tableCellLine = colorPattern(await themeColor(page, "--col-table-cell-line"));
  const bothHead = await themeColor(page, "--col-both-head");
  const bothLine = colorPattern(await themeColor(page, "--col-both-line"));
  const bothCell = await themeColor(page, "--col-both-cell");
  const indexHead = await themeColor(page, "--col-index-head");
  const indexLine = colorPattern(await themeColor(page, "--col-index-line"));
  const indexCell = await themeColor(page, "--col-index-cell");
  await expect(page.locator("#result-table thead th").nth(0)).toHaveCSS("background-color", tableHead);
  await expect(page.locator("#result-table thead th").nth(0)).toHaveCSS("box-shadow", tableLine);
  const headerIcons = await page.locator("#result-table thead th").nth(0).locator(".col-label").evaluate((label) =>
    [...label.children].map((child) => child.dataset.key || child.textContent.trim()),
  );
  expect(headerIcons).toEqual(["id", "hash"]);
  await expect(page.locator("#result-table thead th").nth(1)).toHaveClass(/col-table/);
  await expect(page.locator("#result-table thead th").nth(1).locator('[data-key="range"]')).toBeVisible();
  await expect(page.locator("#result-table tbody td").nth(0)).toHaveCSS("background-color", tableCell);
  await expect(page.locator("#result-table tbody td").nth(0)).toHaveCSS("box-shadow", tableCellLine);

  await replaceEditor(page, `SELECT zebra, apple, id FROM ${name} WHERE id = 'a' AND sk = 1;`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th")).toHaveText(["id", "zebra", "apple"]);

  await replaceEditor(page, `SELECT * FROM ${name} WHERE n = 9 AND sk = 2 USING by-n;`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th")).toHaveText(["id", "sk", "n", "apple", "zebra"]);
  await expect(page.locator("#result-table thead th").nth(0)).toHaveClass(/col-table/);
  await expect(page.locator("#result-table thead th").nth(0)).toHaveCSS("background-color", tableHead);
  await expect(page.locator("#result-table thead th").nth(1)).toHaveClass(/col-both/);
  await expect(page.locator("#result-table thead th").nth(1)).toHaveCSS("background-color", bothHead);
  await expect(page.locator("#result-table thead th").nth(1)).toHaveCSS("box-shadow", bothLine);
  await expect(page.locator("#result-table thead th").nth(1).locator('[data-key="range"]')).toBeVisible();
  await expect(page.locator("#result-table thead th").nth(2)).toHaveClass(/col-index/);
  await expect(page.locator("#result-table thead th").nth(2)).toHaveCSS("background-color", indexHead);
  await expect(page.locator("#result-table thead th").nth(2).locator('[data-key="hash"]')).toBeVisible();
  await expect(page.locator("#result-table tbody td").nth(1)).toHaveCSS("background-color", bothCell);
  await expect(page.locator("#result-table thead th").nth(2)).toHaveCSS("box-shadow", indexLine);
  await expect(page.locator("#result-table tbody td").nth(2)).toHaveCSS("background-color", indexCell);

  await replaceEditor(page, `SELECT * FROM ${name} WHERE apple = 'p' AND id = 'a' USING by-apple;`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th")).toHaveText(["id", "sk", "apple", "n", "zebra"]);
  const shared = page.locator("#result-table thead th").nth(0);
  await expect(shared).toHaveClass(/col-both/);
  await expect(shared).toHaveCSS("background-color", bothHead);
  await expect(shared).toHaveCSS("box-shadow", bothLine);
  await expect(shared.locator('[data-key="hash"]')).toHaveAttribute("aria-label", "Table hash key");
  await expect(shared.locator('[data-key="range"]')).toHaveAttribute("aria-label", "Index range key");
  const sharedIcons = await shared.locator(".col-label").evaluate((label) =>
    [...label.children].map((child) => child.dataset.key || child.textContent.trim()),
  );
  expect(sharedIcons).toEqual(["id", "hash", "range"]);
  await expect(page.locator("#result-table thead th").nth(1)).toHaveClass(/col-table/);
  await expect(page.locator("#result-table thead th").nth(1)).not.toHaveClass(/col-both/);
  const indexHash = page.locator("#result-table thead th").nth(2);
  await expect(indexHash).toHaveClass(/col-index/);
  await expect(indexHash).toHaveCSS("background-color", indexHead);
  await expect(indexHash.locator('[data-key="hash"]')).toHaveAttribute("aria-label", "Index hash key");
  await expect(indexHash.locator('[data-key="range"]')).toHaveCount(0);
  await expect(page.locator("#result-table tbody tr")).toHaveCount(1);
  await expect(page.locator("#result-table tbody td").nth(0)).toHaveText("a");
  await expect(page.locator("#result-table tbody td").nth(2)).toHaveText("p");
  await expect(page.locator("#result-table tbody td").nth(0)).toHaveCSS("background-color", bothCell);
  await expect(page.locator("#result-table tbody td").nth(2)).toHaveCSS("background-color", indexCell);

  await page.locator("#mode-tables").click();
  await page.locator("#table-search").fill(name);
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table thead th")).toHaveText(["id", "sk", "apple", "n", "zebra"]);
  await expect(page.locator("#rows-table thead th").nth(0)).toHaveClass(/col-table/);
  await expect(page.locator("#rows-table thead th").nth(0).locator('[data-key="hash"]')).toBeVisible();
  await expect(page.locator("#rows-table thead th").nth(0)).toHaveCSS("background-color", tableHead);
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

test("opens result and table-browser structure from data already in hand", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  await expect(page.locator(".result-card")).toHaveCount(0);
  await expect(page.locator("#rows-schema")).toBeHidden();

  const name = uniqueTable();
  const sample = `hello ${"wide-sample ".repeat(40)}end`;
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY, sk NUMBER RANGE KEY);`,
      `INSERT INTO ${name} (id, sk, extra) VALUES ('a', 1, '${sample}'), ('b', 2, '${sample}');`,
      `INSERT INTO ${name} (id, sk) VALUES ('c', 3);`,
      `SCAN * FROM ${name};`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th")).toHaveText(["id", "sk", "extra"]);
  const resultCard = page.locator(`.result-card[data-table="${name}"]`);
  await expect(resultCard).toBeVisible();
  await expect(resultCard.locator(".result-card-name")).toHaveText(name);
  await expect(resultCard.locator(".result-schema")).toBeVisible();

  const afterResult = [];
  const watch = (request) => {
    const path = new URL(request.url()).pathname;
    if (path === "/api/tables" || path === "/api/run" || path === "/api/rows") afterResult.push(path);
  };
  page.on("request", watch);
  await resultCard.locator(".result-schema").click();
  const pop = page.locator("#schema-pop");
  await expect(pop).toBeVisible();
  const resultBox = await dialogBox(pop);
  expectFullScreen(resultBox);
  await expect(page.locator("#schema-pop-kicker")).toHaveText("Result structure");
  await expect(page.locator("#schema-pop-info")).toBeVisible();
  await expect(page.locator(".schema-pop-kicker-row #schema-pop-info")).toBeVisible();
  await expect(page.locator("#schema-pop-info")).toHaveAttribute(
    "title",
    "Based on this query result, not the entire table.",
  );
  await expect(page.locator("#schema-pop-title")).toHaveText(name);
  await expect(page.locator(".schema-pop-title-row .schema-key-chip")).toHaveCount(2);
  await expect(page.locator("#schema-pop-summary")).toHaveText("3 columns · 3 rows");
  await expect(page.locator("#schema-pop-query")).toBeVisible();
  await expect(page.locator("#schema-pop-query")).toContainText(`SCAN * FROM ${name}`);
  await expect(page.locator("#schema-table thead th")).toHaveText(["Column", "Type", "Role", "Present", "Sample"]);
  await expectSampleColumnTakesTheRest(page);
  const resultId = page.locator('#schema-table tbody tr[data-column="id"]');
  await expect(resultId).toContainText("id");
  await expect(resultId).toContainText("STRING");
  await expect(resultId).toContainText("Hash key");
  await expect(resultId).toContainText("3/3");
  await expect(resultId.locator(".schema-sample")).not.toHaveText("");
  const resultSk = page.locator('#schema-table tbody tr[data-column="sk"]');
  await expect(resultSk).toContainText("NUMBER");
  await expect(resultSk).toContainText("Range key");
  await expect(resultSk).toContainText("3/3");
  const resultExtra = page.locator('#schema-table tbody tr[data-column="extra"]');
  await expect(resultExtra).toContainText("STRING");
  await expect(resultExtra.locator(".schema-role-muted")).toHaveText("—");
  await expect(resultExtra).toContainText("2/3");
  const sampleCell = resultExtra.locator(".schema-sample");
  await expect(sampleCell).toHaveText(sample);
  const sampleSpace = await sampleCell.evaluate((cell) => ({
    client: cell.clientWidth,
    scroll: cell.scrollWidth,
  }));
  expect(sampleSpace.scroll).toBeLessThanOrEqual(sampleSpace.client + 1);
  const scroller = page.locator(".schema-pop-body");
  const scroll = await scroller.evaluate((node) => ({
    client: node.clientWidth,
    scroll: node.scrollWidth,
  }));
  expect(scroll.scroll).toBeGreaterThan(scroll.client + 8);
  await scroller.evaluate((node) => {
    node.scrollLeft = node.scrollWidth;
  });
  const endVisible = await sampleCell.evaluate((cell) => {
    const box = cell.getBoundingClientRect();
    const parent = cell.closest(".schema-pop-body").getBoundingClientRect();
    return box.right <= parent.right + 2 && box.right >= parent.right - 24;
  });
  expect(endVisible).toBe(true);
  await expect(page.locator("#schema-pop-foot")).toContainText("No extra query was run.");
  expect(afterResult).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(pop).toBeHidden();
  page.off("request", watch);

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(3);
  await expect(page.locator("#rows-schema")).toBeVisible();

  const afterRows = [];
  const watchRows = (request) => {
    const path = new URL(request.url()).pathname;
    if (path === "/api/tables" || path === "/api/run" || path === "/api/rows") afterRows.push(path);
  };
  page.on("request", watchRows);
  await page.locator("#rows-schema").click();
  await expect(pop).toBeVisible();
  const tableBox = await dialogBox(pop);
  expectFullScreen(tableBox);
  await expect(page.locator("#schema-pop-kicker")).toHaveText("Table structure");
  await expect(page.locator("#schema-pop-info")).toBeHidden();
  await expect(page.locator("#schema-pop-title")).toHaveText(name);
  await expect(page.locator(".schema-pop-title-row .schema-key-chip")).toHaveCount(2);
  await expect(page.locator("#schema-pop-summary")).toHaveText("3 columns · 3 rows on this page");
  await expect(page.locator("#schema-pop-query")).toBeHidden();
  const rowsId = page.locator('#schema-table tbody tr[data-column="id"]');
  await expect(rowsId).toContainText("STRING");
  await expect(rowsId).toContainText("Hash key");
  const rowsSk = page.locator('#schema-table tbody tr[data-column="sk"]');
  await expect(rowsSk).toContainText("NUMBER");
  await expect(rowsSk).toContainText("Range key");
  const rowsExtra = page.locator('#schema-table tbody tr[data-column="extra"]');
  await expect(rowsExtra).toContainText("2/3");
  expect(afterRows).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(pop).toBeHidden();
});

test("gives each query result its own named card and structure icon", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const books = uniqueTable();
  const orders = uniqueTable();
  app.trackTable(books);
  app.trackTable(orders);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${books};`,
      `DROP TABLE IF EXISTS ${orders};`,
      `CREATE TABLE ${books} (isbn STRING HASH KEY);`,
      `CREATE TABLE ${orders} (id STRING HASH KEY, n NUMBER);`,
      `INSERT INTO ${books} (isbn, title) VALUES ('978-1', 'DQL Guide');`,
      `INSERT INTO ${orders} (id, n) VALUES ('o1', 4);`,
      `SCAN * FROM ${books};`,
      `SCAN * FROM ${orders};`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await openResultPane(page, "history");
  await expect(page.locator(".result-card")).toHaveCount(2);
  await expect(page.locator("#result-meta")).toBeEmpty();
  await expect(page.locator(".result-card").last().locator(".result-card-meta")).toHaveText("1 row");

  const booksCard = page.locator(`.result-card[data-table="${books}"]`);
  const ordersCard = page.locator(`.result-card[data-table="${orders}"]`);
  await expect(booksCard.locator(".result-card-name")).toHaveText(books);
  await expect(ordersCard.locator(".result-card-name")).toHaveText(orders);
  await expect(booksCard).toContainText("DQL Guide");
  await expect(ordersCard).toContainText("o1");
  await expect(booksCard.locator(".result-schema")).toBeVisible();
  await expect(ordersCard.locator(".result-schema")).toBeVisible();
  await expect(page.locator("#result-table")).toHaveCount(1);

  const extra = [];
  const watch = (request) => {
    const path = new URL(request.url()).pathname;
    if (path === "/api/tables" || path === "/api/run" || path === "/api/rows") extra.push(path);
  };
  page.on("request", watch);
  await booksCard.locator(".result-schema").click();
  await expect(page.locator("#schema-pop")).toBeVisible();
  await expect(page.locator("#schema-pop-title")).toHaveText(books);
  await expect(page.locator("#schema-pop-query")).toContainText(`SCAN * FROM ${books}`);
  await expect(page.locator('#schema-table tbody tr[data-column="isbn"]')).toContainText("STRING");
  await expect(page.locator('#schema-table tbody tr[data-column="isbn"]')).toContainText("Hash key");
  await expect(page.locator('#schema-table tbody tr[data-column="title"]')).toContainText("STRING");
  await page.locator("#schema-pop-close").click();

  await ordersCard.locator(".result-schema").click();
  await expect(page.locator("#schema-pop-title")).toHaveText(orders);
  await expect(page.locator("#schema-pop-query")).toContainText(`SCAN * FROM ${orders}`);
  await expect(page.locator('#schema-table tbody tr[data-column="id"]')).toContainText("STRING");
  await expect(page.locator('#schema-table tbody tr[data-column="n"]')).toContainText("NUMBER");
  expect(extra).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(page.locator("#schema-pop")).toBeHidden();
});

function uniqueTable() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

async function dialogBox(pop) {
  return pop.evaluate((dialog) => {
    const box = dialog.getBoundingClientRect();
    return {
      left: box.left,
      top: box.top,
      width: box.width,
      height: box.height,
      viewWidth: window.innerWidth,
      viewHeight: window.innerHeight,
    };
  });
}

function expectFullScreen(box) {
  expect(box.left).toBeLessThanOrEqual(1);
  expect(box.top).toBeLessThanOrEqual(1);
  expect(box.width).toBeGreaterThanOrEqual(box.viewWidth - 1);
  expect(box.height).toBeGreaterThanOrEqual(box.viewHeight - 1);
}

async function expectSampleColumnTakesTheRest(page) {
  const widths = await page.locator("#schema-table thead th").evaluateAll((cells) =>
    cells.map((cell) => cell.getBoundingClientRect().width),
  );
  expect(widths).toHaveLength(5);
  const content = widths.slice(0, 4).reduce((sum, width) => sum + width, 0);
  expect(widths[4]).toBeGreaterThan(content);
}
