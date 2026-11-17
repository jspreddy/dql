import fs from "node:fs";
import path from "node:path";
import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
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
  expect(heading).toEqual(["side-title", "side-tools"]);
  const tools = await page.locator("#side-tools").evaluate((row) =>
    [...row.children].map((child) => child.id),
  );
  expect(tools).toEqual(["tree-fold", "new-file", "refresh-tables", "table-cache"]);
  await expect(page.locator("#file-filters")).toBeHidden();
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
  await expect(page.locator("#settings-cache-age")).toHaveAttribute("data-tip", /.+/);

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
  await expect(page.locator("#settings-cache-age")).toHaveAttribute("data-tip", "No table list cached.");
  await expect(page.locator("#settings-cache-age")).not.toHaveAttribute("title");
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

test("pages a table fifty rows at a time from a searched name", async ({ page, app }, testInfo) => {
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
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "51 rows inserted" })).toBeVisible();

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
  await expect(page.locator("#table-query .table-query-actions #table-query-pager")).toBeVisible();
  // Run and the pager share one bar in the controls pane: Run at the left, the pager at the right.
  // The query as built sits in the right-hand pane.
  const dqlBox = await page.locator("#table-query-dql").boundingBox();
  const barBox = await page.locator("#table-query-actions").boundingBox();
  const runBox = await page.locator("#table-query-run").boundingBox();
  const pagerBox = await page.locator("#table-query-pager").boundingBox();
  expect(dqlBox.x).toBeGreaterThanOrEqual(barBox.x + barBox.width);
  expect(runBox.x - barBox.x).toBeLessThan(4);
  expect(Math.abs(barBox.x + barBox.width - (pagerBox.x + pagerBox.width))).toBeLessThan(4);
  expect(Math.abs(runBox.y + runBox.height / 2 - (pagerBox.y + pagerBox.height / 2))).toBeLessThan(4);
  await expect(page.locator("#tables-view > .pager")).toHaveCount(0);
  await expect(page.locator("#page-label")).toHaveText("Showing 1–50");
  await expect(page.locator("#page-prev")).toBeDisabled();
  await expect(page.locator("#page-next")).toBeEnabled();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(50);

  await page.locator("#table-search").fill("zzz_no_such_table_*");
  await expect(page.locator("#table-list")).toContainText("No tables match zzz_no_such_table_*");

  await page.locator("#table-search").fill(name);
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#page-label")).toHaveText("Showing 1–50");
  const rowPages = trackRowPages(page);
  await page.locator("#page-next").click();
  await expect(page.locator("#page-label")).toHaveText("Showing 51–51");
  await expect(page.locator("#page-next")).toBeDisabled();
  await expect(page.locator("#page-prev")).toBeEnabled();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(1);
  expect(rowPages.splice(0)).toEqual([1]);

  await page.locator("#page-prev").click();
  await expect(page.locator("#page-label")).toHaveText("Showing 1–50");
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(50);
  expect(rowPages).toEqual([]);
  // Next to Run: the page's rows and the read's time (and capacity when it used any) as pills.
  await expect(page.locator('[data-testid="table-query-count"]')).toHaveText("50 rows");
  await expect(page.locator('[data-testid="table-query-time"]')).toHaveText(/^\d+(\.\d+)? m?s$/);
  await expect(page.locator("#table-query-cancel")).toBeHidden();
  const runRight = await page.locator("#table-query-run").boundingBox();
  const countBox = await page.locator('[data-testid="table-query-count"]').boundingBox();
  expect(countBox.x).toBeGreaterThan(runRight.x + runRight.width);
  await page.screenshot({ path: playwrightArtifact(`table-pager-toolbar-${testInfo.project.name}.png`) });

  // While a page loads, Cancel takes the pills' place; it drops the read and stops it on the server.
  let release;
  const held = new Promise((resolve) => { release = resolve; });
  await page.route("**/api/rows?*", async (route) => {
    await held;
    await route.continue().catch(() => {});
  });
  const stop = page.waitForRequest((request) => new URL(request.url()).pathname === "/api/stop");
  // A new read (a filter) is not cached, so it goes to the server.
  await page.locator("#table-query-add-filter").click();
  const filterRow = page.locator('[data-testid="table-query-filter-row"]');
  await filterRow.locator('[data-testid="table-query-attr"]').fill("id");
  await filterRow.locator('[data-testid="table-query-value"]').fill("r00");
  await page.locator("#table-query-run").click();
  await expect(page.locator("#table-query-cancel")).toBeVisible();
  await expect(page.locator("#table-query-stats")).toBeHidden();
  await page.screenshot({ path: playwrightArtifact(`table-run-cancel-${testInfo.project.name}.png`) });
  await page.locator("#table-query-cancel").click();
  await stop;
  await expect(page.locator("#rows-body")).toHaveText("Read cancelled.");
  await expect(page.locator("#table-query-cancel")).toBeHidden();
  release();
  await page.unroute("**/api/rows?*");
});

test("pages a thousand numeric ids forward and back", async ({ page, app }, testInfo) => {
  test.setTimeout(120_000);
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  const tuples = Array.from({ length: 1000 }, (_, index) => `('g', ${index + 1})`).join(", ");
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (gid STRING HASH KEY, id NUMBER RANGE KEY);`,
      `INSERT INTO ${name} (gid, id) VALUES ${tuples};`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "1000 rows inserted" })).toBeVisible({
    timeout: 60_000,
  });

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await page.locator("#table-query-query").click();
  await page.locator('[data-testid="table-query-key-row"] [data-testid="table-query-value"]').fill("g");
  await expect(page.locator("#table-query-dql")).toHaveText(`SELECT * FROM ${name} WHERE gid = 'g'`);
  const selectPages = trackRowPages(page, { dql: "SELECT" });
  const queried = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return response.ok() && url.pathname === "/api/rows" && (url.searchParams.get("dql") || "").includes("SELECT");
  });
  await page.locator("#table-query-run").click();
  await queried;

  const gotoPage = async (from, to) => {
    const step = to > from ? 1 : -1;
    const button = step > 0 ? "#page-next" : "#page-prev";
    for (let current = from; current !== to; current += step) {
      const next = current + step;
      const start = next * 50 + 1;
      const end = Math.min(start + 49, 1000);
      await page.locator(button).click();
      await expect(page.locator("#page-label")).toHaveText(`Showing ${start}–${end}`);
    }
  };

  await expectNumericPage(page, 0);
  await page.screenshot({ path: playwrightArtifact(`table-pager-thousand-start-${testInfo.project.name}.png`) });
  await gotoPage(0, 1);
  await expectNumericPage(page, 1);
  await gotoPage(1, 10);
  await expectNumericPage(page, 10);
  await page.screenshot({ path: playwrightArtifact(`table-pager-thousand-middle-${testInfo.project.name}.png`) });
  await gotoPage(10, 9);
  await expectNumericPage(page, 9);
  await gotoPage(9, 19);
  await expectNumericPage(page, 19);
  await page.screenshot({ path: playwrightArtifact(`table-pager-thousand-end-${testInfo.project.name}.png`) });
  await gotoPage(19, 18);
  await expectNumericPage(page, 18);
  await gotoPage(18, 0);
  await expectNumericPage(page, 0);
  expect(selectPages).toEqual(Array.from({ length: 20 }, (_, index) => index));

  const rerun = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return response.ok() && url.pathname === "/api/rows" && url.searchParams.get("page") === "0"
      && (url.searchParams.get("dql") || "").includes("SELECT");
  });
  await page.locator("#table-query-run").click();
  await rerun;
  await expectNumericPage(page, 0);
  expect(selectPages).toEqual([...Array.from({ length: 20 }, (_, index) => index), 0]);
  fs.writeFileSync(
    playwrightArtifact(`table-rows-cache-pages-${testInfo.project.name}.json`),
    JSON.stringify(selectPages),
  );
});

test("table column headers stay put while rows scroll", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  const tuples = Array.from({ length: 40 }, (_, index) => `('r${String(index).padStart(2, "0")}', ${index}, 'note')`).join(", ");
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY, n NUMBER, note STRING);`,
      `INSERT INTO ${name} (id, n, note) VALUES ${tuples};`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "40 rows inserted" })).toBeVisible();

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table thead th:not(.row-peek)")).toHaveText(["id", "n", "note"]);
  const hashHeader = page.locator("#rows-table thead th:not(.row-peek)").nth(0);
  await expect(hashHeader).toHaveClass(/col-table/);
  await expect(hashHeader).toHaveCSS("position", "sticky");

  const scroller = page.locator("#rows-body");
  const before = await headerTops(page);
  const firstRow = await page.locator("#rows-table tbody tr").first().boundingBox();
  await scroller.evaluate((node) => {
    node.scrollTop = 160;
  });
  await expect.poll(async () => scroller.evaluate((node) => node.scrollTop)).toBeGreaterThan(100);
  const after = await headerTops(page);
  expect(after).toEqual(before);
  const movedRow = await page.locator("#rows-table tbody tr").first().boundingBox();
  expect(firstRow.y - movedRow.y).toBeGreaterThan(100);
});

test("row peek stays on the left edge while the table scrolls sideways", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  const columns = Array.from({ length: 8 }, (_, index) => `wide_${index}`);
  const values = columns.map((column) => `'${column}-${"x".repeat(24)}'`);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY);`,
      `INSERT INTO ${name} (id, ${columns.join(", ")}) VALUES ('row-1', ${values.join(", ")});`,
      `SELECT * FROM ${name} WHERE id = 'row-1';`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table tbody tr")).toContainText("row-1");

  const resultScroller = page.locator(".result-card-body");
  const resultPeek = page.locator("#result-table [data-testid='row-peek']");
  await resultPeek.hover();
  const resultBefore = await edgeOffset(resultPeek);
  await resultScroller.evaluate((node) => {
    node.scrollLeft = node.scrollWidth;
  });
  await expect.poll(async () => resultScroller.evaluate((node) => node.scrollLeft)).toBeGreaterThan(80);
  const resultAfter = await edgeOffset(resultPeek);
  expect(Math.abs(resultAfter - resultBefore)).toBeLessThanOrEqual(1);
  await resultPeek.click();
  await expect(page.locator("#row-pop")).toBeVisible();
  await page.keyboard.press("Escape");

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator("#rows-table tbody tr")).toContainText("row-1");
  const rowsScroller = page.locator("#rows-body");
  const rowsPeek = page.locator("#rows-table [data-testid='row-peek']");
  await rowsPeek.hover();
  const rowsBefore = await edgeOffset(rowsPeek);
  await rowsScroller.evaluate((node) => {
    node.scrollLeft = node.scrollWidth;
  });
  await expect.poll(async () => rowsScroller.evaluate((node) => node.scrollLeft)).toBeGreaterThan(80);
  const rowsAfter = await edgeOffset(rowsPeek);
  expect(Math.abs(rowsAfter - rowsBefore)).toBeLessThanOrEqual(1);
  await expect(rowsPeek).toHaveCSS("position", "absolute");
  await expect(page.locator("#rows-table td.row-peek")).toHaveCSS("position", "sticky");
  await rowsPeek.click();
  await expect(page.locator("#row-pop")).toBeVisible();
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
  await expect(page.locator("#result-table thead th:not(.row-peek)")).toHaveText(["id", "sk", "apple", "n", "zebra"]);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(0)).toHaveClass(/col-table/);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(0).locator('[data-key="hash"]')).toBeVisible();
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
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(0)).toHaveCSS("background-color", tableHead);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(0)).toHaveCSS("box-shadow", tableLine);
  const headerIcons = await page.locator("#result-table thead th:not(.row-peek)").nth(0).locator(".col-label").evaluate((label) =>
    [...label.children].map((child) => child.dataset.key || child.textContent.trim()),
  );
  expect(headerIcons).toEqual(["id", "hash"]);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(1)).toHaveClass(/col-table/);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(1).locator('[data-key="range"]')).toBeVisible();
  await expect(page.locator("#result-table tbody td:not(.row-peek)").nth(0)).toHaveCSS("background-color", tableCell);
  await expect(page.locator("#result-table tbody td:not(.row-peek)").nth(0)).toHaveCSS("box-shadow", tableCellLine);

  await replaceEditor(page, `SELECT zebra, apple, id FROM ${name} WHERE id = 'a' AND sk = 1;`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th:not(.row-peek)")).toHaveText(["id", "zebra", "apple"]);

  await replaceEditor(page, `SELECT * FROM ${name} WHERE n = 9 AND sk = 2 USING by-n;`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th:not(.row-peek)")).toHaveText(["id", "sk", "n", "apple", "zebra"]);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(0)).toHaveClass(/col-table/);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(0)).toHaveCSS("background-color", tableHead);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(1)).toHaveClass(/col-both/);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(1)).toHaveCSS("background-color", bothHead);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(1)).toHaveCSS("box-shadow", bothLine);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(1).locator('[data-key="range"]')).toBeVisible();
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(2)).toHaveClass(/col-index/);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(2)).toHaveCSS("background-color", indexHead);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(2).locator('[data-key="hash"]')).toBeVisible();
  await expect(page.locator("#result-table tbody td:not(.row-peek)").nth(1)).toHaveCSS("background-color", bothCell);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(2)).toHaveCSS("box-shadow", indexLine);
  await expect(page.locator("#result-table tbody td:not(.row-peek)").nth(2)).toHaveCSS("background-color", indexCell);

  await replaceEditor(page, `SELECT * FROM ${name} WHERE apple = 'p' AND id = 'a' USING by-apple;`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table thead th:not(.row-peek)")).toHaveText(["id", "sk", "apple", "n", "zebra"]);
  const shared = page.locator("#result-table thead th:not(.row-peek)").nth(0);
  await expect(shared).toHaveClass(/col-both/);
  await expect(shared).toHaveCSS("background-color", bothHead);
  await expect(shared).toHaveCSS("box-shadow", bothLine);
  await expect(shared.locator('[data-key="hash"]')).toHaveAttribute("aria-label", "Table hash key");
  await expect(shared.locator('[data-key="range"]')).toHaveAttribute("aria-label", "Index range key");
  const sharedIcons = await shared.locator(".col-label").evaluate((label) =>
    [...label.children].map((child) => child.dataset.key || child.textContent.trim()),
  );
  expect(sharedIcons).toEqual(["id", "hash", "range"]);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(1)).toHaveClass(/col-table/);
  await expect(page.locator("#result-table thead th:not(.row-peek)").nth(1)).not.toHaveClass(/col-both/);
  const indexHash = page.locator("#result-table thead th:not(.row-peek)").nth(2);
  await expect(indexHash).toHaveClass(/col-index/);
  await expect(indexHash).toHaveCSS("background-color", indexHead);
  await expect(indexHash.locator('[data-key="hash"]')).toHaveAttribute("aria-label", "Index hash key");
  await expect(indexHash.locator('[data-key="range"]')).toHaveCount(0);
  await expect(page.locator("#result-table tbody tr")).toHaveCount(1);
  await expect(page.locator("#result-table tbody td:not(.row-peek)").nth(0)).toHaveText("a");
  await expect(page.locator("#result-table tbody td:not(.row-peek)").nth(2)).toHaveText("p");
  await expect(page.locator("#result-table tbody td:not(.row-peek)").nth(0)).toHaveCSS("background-color", bothCell);
  await expect(page.locator("#result-table tbody td:not(.row-peek)").nth(2)).toHaveCSS("background-color", indexCell);

  await page.locator("#mode-tables").click();
  await page.locator("#table-search").fill(name);
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table thead th:not(.row-peek)")).toHaveText(["id", "sk", "apple", "n", "zebra"]);
  await expect(page.locator("#rows-table thead th:not(.row-peek)").nth(0)).toHaveClass(/col-table/);
  await expect(page.locator("#rows-table thead th:not(.row-peek)").nth(0).locator('[data-key="hash"]')).toBeVisible();
  await expect(page.locator("#rows-table thead th:not(.row-peek)").nth(0)).toHaveCSS("background-color", tableHead);
  await expect(page.locator("#rows-table thead th:not(.row-peek)").nth(1).locator('[data-key="range"]')).toBeVisible();
  await expect(page.locator("#rows-table thead th:not(.row-peek)").nth(3)).not.toHaveClass(/col-/);
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

test("ST lists tables the same way as SHOW TABLES", async ({ page, app }) => {
  const name = uniqueTable();
  app.trackTable(name);
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  await replaceEditor(page, `CREATE TABLE ${name} (id STRING HASH KEY);`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="run-status-ok"]')).toHaveCount(1);

  const listed = async (statement) => {
    await replaceEditor(page, statement);
    await pressMod(page, "A");
    // The previous statement's card and its ok mark are still on the page right after the
    // click. Mark the old cards, so only this run's card can be read.
    await page.locator(".result-card").evaluateAll((cards) => cards.forEach((card) => card.setAttribute("data-stale", "")));
    await page.locator("#run").click();
    const card = page.locator(".result-card:not([data-stale])").last();
    await expect(card).toContainText(name);
    await expect(page.locator('[data-testid="run-status-ok"]')).toHaveCount(1);
    return card.locator("table tbody tr").allInnerTexts();
  };
  const fromShow = await listed(`SHOW TABLES LIKE '${name}';`);
  const fromSt = await listed(`ST LIKE '${name}';`);
  expect(fromSt).toEqual(fromShow);
  expect(fromSt).toHaveLength(1);
  // ST 'pattern' is ST LIKE 'pattern'; a bare name lists names that contain it.
  expect(await listed(`st '${name}';`)).toEqual(fromShow);
  const middle = name.slice(3, -2);
  expect(await listed(`st ${middle};`)).toEqual(await listed(`SHOW TABLES LIKE '%${middle}%';`));

  // ST is a keyword to the editor: colored like SHOW, and lint accepts it (a lint error
  // first, so the status clearing shows lint ran on "st;").
  await replaceEditor(page, "SELEKT;");
  await expect(page.locator("#lint-status")).toHaveText("1 error");
  await replaceEditor(page, "st;");
  await expect(page.locator("#lint-status")).toBeHidden();
  const color = await page.evaluate(() => {
    const span = [...document.querySelectorAll(".cm-content span")].find((el) => el.textContent === "st");
    return span ? getComputedStyle(span).color : "";
  });
  expect(color).toBe(await themeColor(page, "--tok-keyword"));
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
  await expect(similar).toHaveAttribute("data-tip", `No exact match for "${stem}", so showing similar names.`);
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
  await expect(related).toHaveAttribute("data-tip", `No exact match for "${key}", so showing related keys.`);
  await expect(page.locator(`#table-${orders}`)).toBeVisible();
  await expect(page.locator(`#table-${shipments}`)).toBeVisible();
  await expect(page.locator(`#table-${shipments}`)).toContainText(key);
  await expect(page.locator(`#table-${catalog}`)).toHaveCount(0);

  await page.locator("#table-search").fill("zzz_no_such_table_*");
  await expect(page.locator("#table-list")).toContainText("No tables match zzz_no_such_table_*");
  await expect(page.locator("#table-list [data-testid='intelligent-match']")).toHaveCount(0);
});

test("opens result and table-browser structure from data already in hand", async ({ page, app }, testInfo) => {
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
  await expect(page.locator("#result-table thead th:not(.row-peek)")).toHaveText(["id", "sk", "extra"]);
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
    "data-tip",
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
  const definition = page.locator("[data-testid='schema-definition']");
  await expect(definition.locator(":scope > h3")).toHaveText("Definition");
  await expect(definition).toContainText("From the schema cache");
  const createText = definition.locator("[data-testid='schema-create-query'] pre");
  await expect(createText).toContainText(`CREATE TABLE ${name} (`);
  await expect(createText).toContainText("id STRING HASH KEY,");
  await expect(createText).toContainText("sk NUMBER RANGE KEY");
  expect((await createText.innerText()).split("\n").length).toBeGreaterThan(2);
  const columns = definition.locator("[data-testid='schema-definition-columns']");
  await expect(columns.locator("th")).toHaveText(["Column", "Type", "Role", "Index"]);
  const definedId = columns.locator("tbody tr[data-column='id']");
  await expect(definedId).toContainText("STRING");
  await expect(definedId).toContainText("Hash key");
  const definedSk = columns.locator("tbody tr[data-column='sk']");
  await expect(definedSk).toContainText("NUMBER");
  await expect(definedSk).toContainText("Range key");
  await expect(columns.locator("tbody tr[data-column='extra']")).toHaveCount(0);
  const result = page.locator("[data-testid='schema-result']");
  await expect(result.locator("h3")).toHaveText("Result structure");
  const resultNote = result.locator(".note-card");
  await expect(resultNote).toHaveAttribute("data-variant", "info");
  await expect(resultNote).toHaveText("Types and metrics come from the data fetched so far. No extra queries were run, so this is a partial structure.");
  await expect(resultNote.locator("use")).toHaveAttribute("href", /\/vendor\/icons\/info\.svg#icon$/);
  const variants = await page.evaluate(async () => {
    const { noteCard } = await import("/note-card.js");
    const host = document.createElement("div");
    host.hidden = true;
    document.body.append(host);
    const listed = ["error", "success", "alert", "info"].map((variant) => {
      const card = noteCard(variant + " note", variant);
      host.append(card);
      const style = getComputedStyle(card);
      return {
        variant: card.dataset.variant,
        text: card.querySelector("p").textContent,
        icon: card.querySelector("use").getAttribute("href"),
        background: style.backgroundColor,
        border: style.borderTopColor,
        color: style.color,
      };
    });
    host.remove();
    return listed;
  });
  expect(variants.map((card) => card.variant)).toEqual(["error", "success", "alert", "info"]);
  expect(variants.map((card) => card.icon)).toEqual([
    "/vendor/icons/error.svg#icon",
    "/vendor/icons/check_circle.svg#icon",
    "/vendor/icons/warning.svg#icon",
    "/vendor/icons/info.svg#icon",
  ]);
  expect(new Set(variants.map((card) => card.background)).size).toBe(4);
  expect(new Set(variants.map((card) => card.color)).size).toBe(4);
  expect(new Set(variants.map((card) => card.border)).size).toBe(4);
  await expect(page.locator("#schema-pop-foot")).toBeHidden();
  await expect(page.locator("#schema-pop h3")).toHaveText(["Definition", "Result structure"]);
  await page.screenshot({ path: playwrightArtifact(`table-structure-${testInfo.project.name}.png`) });

  // Definition and Result structure scroll as one surface inside the dialog body.
  await expect(page.locator("#schema-pop-schema")).toHaveCount(0);
  await page.setViewportSize({ width: 1280, height: 480 });
  const scrolled = await page.evaluate(() => {
    const scroller = (el) => {
      for (let node = el.parentElement; node; node = node.parentElement) {
        const overflow = getComputedStyle(node).overflowY;
        if ((overflow === "auto" || overflow === "scroll") && node.scrollHeight > node.clientHeight) return node;
      }
      return null;
    };
    const definitionEl = document.querySelector("[data-testid='schema-definition']");
    const resultEl = document.querySelector("[data-testid='schema-result']");
    const body = document.querySelector("#schema-pop-body");
    const before = [definitionEl.getBoundingClientRect().top, resultEl.getBoundingClientRect().top];
    body.scrollTop = 120;
    const after = [definitionEl.getBoundingClientRect().top, resultEl.getBoundingClientRect().top];
    return {
      definitionScroller: scroller(definitionEl)?.id,
      resultScroller: scroller(resultEl)?.id,
      scrollTop: body.scrollTop,
      moved: [before[0] - after[0], before[1] - after[1]],
    };
  });
  expect(scrolled.definitionScroller).toBe("schema-pop-body");
  expect(scrolled.resultScroller).toBe("schema-pop-body");
  expect(scrolled.scrollTop).toBe(120);
  expect(scrolled.moved).toEqual([120, 120]);
  await page.screenshot({ path: playwrightArtifact(`table-structure-scrolled-${testInfo.project.name}.png`) });
  await page.locator("#schema-pop-body").evaluate((node) => {
    node.scrollTop = 0;
  });
  await page.setViewportSize({ width: 1280, height: 800 });
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

test("a local table's definition is cached for the linter and the structure view", async ({ page, app }, testInfo) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  const name = uniqueTable();
  fs.writeFileSync(path.join(app.workspace, "queries", "people.csv"), "id,name\na,ada\nb,grace\n");

  await replaceEditor(page, `DROP LOCALTABLE IF EXISTS ${name};\nCREATE LOCALTABLE ${name} AS './people.csv';`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="run-status-ok"]')).toHaveCount(2);

  // The linter uses the local table's definition: an unkeyed SELECT is an error, not a
  // "no cached definition" warning, and a SCAN then clears the status.
  await replaceEditor(page, `SELECT * FROM ${name};`);
  await expect(page.locator("#lint-status")).toHaveText("1 error");
  await expect(page.locator("#lint-status")).toHaveAttribute("data-tip", /indexed WHERE/);
  await replaceEditor(page, `SCAN * FROM ${name};`);
  await expect(page.locator("#lint-status")).toBeHidden();

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await page.locator(`#table-${name}`).click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(2);
  await page.locator("#rows-schema").click();
  const definition = page.locator("[data-testid='schema-definition']");
  await expect(definition).toContainText("From this local table's file");
  await expect(definition.locator("[data-testid='schema-create-query'] pre")).toHaveText(
    `CREATE LOCALTABLE ${name} AS '${path.join(app.workspace, "queries", "people.csv")}';`,
  );
  const columns = definition.locator("[data-testid='schema-definition-columns']");
  await expect(columns.locator("tbody tr[data-column='id']")).toContainText("Hash key");
  await expect(columns.locator("tbody tr[data-column='name']")).toContainText("STRING");
  await page.screenshot({ path: playwrightArtifact(`local-table-structure-${testInfo.project.name}.png`) });
  await page.keyboard.press("Escape");

  await page.locator("#mode-query").click();
  await replaceEditor(page, `DROP LOCALTABLE ${name};`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="run-status-ok"]')).toHaveCount(1);
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

test("scrolls a wide result table sideways inside the card", async ({ page, app }, testInfo) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  const columns = Array.from({ length: 14 }, (_, index) => `col_${String(index + 1).padStart(2, "0")}`);
  const values = columns.map((column) => `'${column}-value'`);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY);`,
      `INSERT INTO ${name} (id, ${columns.join(", ")}) VALUES ('row-1', ${values.join(", ")});`,
      `SELECT * FROM ${name} WHERE id = 'row-1';`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();

  const card = page.locator(`.result-card[data-table="${name}"]`);
  const scroller = card.locator(".result-card-body");
  await expect(card).toBeVisible();
  await expect(page.locator("#result-table thead th:not(.row-peek)")).toHaveText(["id", ...columns]);
  await expect(page.locator("#result-table")).toContainText("col_14-value");

  const paneBox = await page.locator("#result-body").boundingBox();
  const cardBox = await card.boundingBox();
  expect(cardBox.x).toBeGreaterThanOrEqual(paneBox.x - 1);
  expect(cardBox.x + cardBox.width).toBeLessThanOrEqual(paneBox.x + paneBox.width + 1);

  const metrics = await scroller.evaluate((node) => ({
    client: node.clientWidth,
    scroll: node.scrollWidth,
    overflowX: getComputedStyle(node).overflowX,
  }));
  expect(metrics.overflowX).toBe("auto");
  expect(metrics.scroll).toBeGreaterThan(metrics.client + 24);

  const last = page.locator("#result-table thead th:not(.row-peek)").last();
  const clipped = await last.evaluate((cell) => {
    const box = cell.getBoundingClientRect();
    const parent = cell.closest(".result-card-body").getBoundingClientRect();
    return box.right > parent.right + 8;
  });
  expect(clipped).toBe(true);
  await card.screenshot({
    path: playwrightArtifact(`result-table-scroll-start-${testInfo.project.name}.png`),
  });

  await scroller.hover();
  await page.mouse.wheel(480, 0);
  await expect.poll(async () => scroller.evaluate((node) => node.scrollLeft)).toBeGreaterThan(20);

  await scroller.evaluate((node) => {
    node.scrollLeft = node.scrollWidth;
  });
  const endVisible = await last.evaluate((cell) => {
    const box = cell.getBoundingClientRect();
    const parent = cell.closest(".result-card-body").getBoundingClientRect();
    return box.right <= parent.right + 2 && box.left < parent.right - 8;
  });
  expect(endVisible).toBe(true);
  await card.screenshot({
    path: playwrightArtifact(`result-table-scroll-end-${testInfo.project.name}.png`),
  });
});

test("table browser scan and query pickers filter without typing DQL", async ({ page, app }, testInfo) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY, sk NUMBER RANGE KEY) GLOBAL INDEX ('by-status', status STRING, sk NUMBER);`,
      `INSERT INTO ${name} (id, sk, status) VALUES ('ada', 1, 'open'), ('ada', 2, 'shut'), ('tom', 3, 'open');`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "3 rows inserted" })).toBeVisible();

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(3);

  const picker = page.locator("#table-query");
  await expect(picker).toBeVisible();
  await expect(page.locator("#table-query-scan")).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#table-query-query")).toHaveAttribute("aria-pressed", "false");
  await expect(page.locator("#table-query-forward")).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#table-query-source")).toHaveValue("");
  await expect(page.locator("#table-query-dql")).toHaveText(`SCAN * FROM ${name}`);
  await expect(page.locator("#table-query-source")).toContainText("by-status");
  const scanLead = await modeGroupBox(page);
  expect(scanLead.sourceLeft).toBeGreaterThanOrEqual(scanLead.buttonsRight - 1);
  expect(scanLead.sourceWidth).toBe(380);

  await page.locator("#table-query-add-filter").click();
  const filter = page.locator('[data-testid="table-query-filter-row"]');
  await expect(filter).toBeVisible();
  const attr = filter.locator('[data-testid="table-query-attr"]');
  await expect(attr).toHaveValue("");
  await expect(filter.locator("datalist option[value=id]")).toHaveCount(1);
  await expect(filter.locator("datalist option[value=sk]")).toHaveCount(1);
  await expect(filter.locator("datalist option[value=status]")).toHaveCount(1);
  await attr.fill("status");
  await filter.locator('[data-testid="table-query-value"]').fill("open");
  await expect(page.locator("#table-query-dql")).toHaveText(`SCAN * FROM ${name} WHERE status = 'open'`);
  await expect(page.locator("#table-query-run")).toHaveClass(/is-pending/);
  // Run looks and reads like the editor's Run, and Ctrl+Enter (⌘+Enter on a Mac) runs from a filter.
  const runLabel = (selector) => page.locator(selector).evaluate((el) => el.textContent.replace(/\s+/g, " ").trim());
  expect(await runLabel("#table-query-run")).toBe(await runLabel("#run"));
  expect(await runLabel("#table-query-run")).toMatch(/^(Ctrl|⌘) \+ Enter$/);
  await expect(page.locator("#table-query-run")).toHaveAttribute("aria-keyshortcuts", /^(Control|Meta)\+Enter$/);
  await filter.locator('[data-testid="table-query-value"]').press("ControlOrMeta+Enter");
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(2);
  await expect(page.locator("#table-query-run")).not.toHaveClass(/is-pending/);
  await expect(page.locator("#rows-table tbody")).toContainText("ada");
  await expect(page.locator("#rows-table tbody")).toContainText("tom");
  await expect(page.locator("#rows-table tbody")).not.toContainText("shut");

  await page.locator("#table-query-query").click();
  await expect(page.locator("#table-query-query")).toHaveAttribute("aria-pressed", "true");
  const queryLead = await modeGroupBox(page);
  expect(queryLead.left).toBe(scanLead.left);
  expect(queryLead.top).toBe(scanLead.top);
  expect(Math.abs(queryLead.sourceLeft - scanLead.sourceLeft)).toBeLessThanOrEqual(1);
  expect(queryLead.sourceWidth).toBe(scanLead.sourceWidth);
  const key = page.locator('[data-testid="table-query-key-row"]');
  await expect(key).toHaveCount(1);
  await expect(key.locator('[data-testid="table-query-attr"]')).toHaveValue("id");
  await expect(page.locator("#table-query-run")).toBeDisabled();
  await expect(page.locator("#table-query-error")).toHaveText("Query needs a hash key value.");
  await key.locator('[data-testid="table-query-value"]').fill("ada");
  await expect(page.locator("#table-query-dql")).toHaveText(`SELECT * FROM ${name} WHERE id = 'ada' AND status = 'open'`);
  await page.locator("#table-query-run").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(1);
  await expect(page.locator("#rows-table tbody td:not(.row-peek)").nth(0)).toHaveText("ada");
  await expect(page.locator("#rows-table tbody")).toContainText("open");

  await page.locator("#table-query-add-key").click();
  const range = page.locator('[data-testid="table-query-key-row"]').nth(1);
  await range.locator('[data-testid="table-query-op"]').selectOption("eq");
  await range.locator('[data-testid="table-query-value"]').fill("1");
  await page.locator("#table-query-reverse").click();
  await expect(page.locator("#table-query-dql")).toHaveText(
    `SELECT * FROM ${name} WHERE id = 'ada' AND sk = 1 AND status = 'open' DESC`,
  );
  await page.locator("#table-query-run").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(1);
  await expect(page.locator("#rows-table tbody td:not(.row-peek)").nth(1)).toHaveText("1");

  await page.locator("#table-query-source").selectOption("by-status");
  const indexLead = await modeGroupBox(page);
  expect(Math.abs(indexLead.sourceLeft - scanLead.sourceLeft)).toBeLessThanOrEqual(1);
  expect(indexLead.sourceWidth).toBe(scanLead.sourceWidth);
  await expect(page.locator('[data-testid="table-query-key-row"] [data-testid="table-query-attr"]')).toHaveValue("status");
  await page.locator('[data-testid="table-query-key-row"] [data-testid="table-query-value"]').fill("open");
  await page.locator('[data-testid="table-query-filter-row"] [data-testid="table-query-remove"]').click();
  await expect(page.locator("#table-query-dql")).toHaveText(
    `SELECT * FROM ${name} WHERE status = 'open' USING by-status DESC`,
  );
  await page.locator("#table-query-run").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(2);
  await expect(page.locator("#rows-table tbody td:not(.row-peek)").nth(1)).toHaveText("3");
  await page.screenshot({ path: playwrightArtifact(`table-query-picker-${testInfo.project.name}.png`) });
});

test("table browser suggests structure attributes and accepts a typed name", async ({ page, app }, testInfo) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY);`,
      `INSERT INTO ${name} (id, extra) VALUES ('ada', 'hello'), ('tom', 'world');`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "2 rows inserted" })).toBeVisible();

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(2);

  await page.locator("#table-query-add-filter").click();
  const filter = page.locator('[data-testid="table-query-filter-row"]');
  const attr = filter.locator('[data-testid="table-query-attr"]');
  await expect(attr).toHaveValue("");
  await expect(filter.locator("datalist option[value=id]")).toHaveCount(1);
  await attr.fill("ghost");
  await filter.locator('[data-testid="table-query-value"]').fill("nope");
  await expect(page.locator("#table-query-dql")).toHaveText(`SCAN * FROM ${name} WHERE ghost = 'nope'`);
  await page.locator("#table-query-run").click();
  await expect(page.locator("#page-label")).toHaveText("No rows");

  await attr.fill("extra");
  await filter.locator('[data-testid="table-query-value"]').fill("hello");
  await expect(page.locator("#table-query-dql")).toHaveText(`SCAN * FROM ${name} WHERE extra = 'hello'`);
  await page.locator("#table-query-run").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(1);
  await expect(page.locator("#rows-table tbody")).toContainText("ada");
  await expect(page.locator("#rows-table tbody")).not.toContainText("tom");
  await page.screenshot({ path: playwrightArtifact(`table-query-custom-attr-${testInfo.project.name}.png`) });
});

test("table browser runs every DynamoDB condition function", async ({ page, app }, testInfo) => {
  test.setTimeout(90_000);
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY, sk STRING RANGE KEY);`,
      `INSERT INTO ${name} (id='store-1', sk='ITEM#mug', name='hello world', title='Blue mug', tags=('lighting', 'ceramic'), items=['handle', 'glaze'], qty=3);`,
      `INSERT INTO ${name} (id='store-1', sk='ITEM#lamp', name='goodbye', title='Arc lamp', tags=('lighting', 'metal', 'floor'), items=['shade', 'cord'], qty=1);`,
      `INSERT INTO ${name} (id='store-1', sk='NOTE#1', name='memo', title='Field notes');`,
      `INSERT INTO ${name} (id='store-2', sk='ITEM#cable', name='usb world', title='USB-C cable', tags=('cable'), items=[], qty=12);`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await openResultPane(page, "history");
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "one row inserted" })).toHaveCount(4);

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator(`#table-${name}`)).toBeVisible();

  await page.locator("#table-query-add-filter").click();
  const filter = page.locator('[data-testid="table-query-filter-row"]');

  const runFilter = async ({ attr, op, value, sizeOp, type, dql, count, has, missing }) => {
    await filter.locator('[data-testid="table-query-op"]').selectOption(op);
    await filter.locator('[data-testid="table-query-attr"]').fill(attr);
    if (type) await filter.locator('[data-testid="table-query-type"]').selectOption(type);
    if (sizeOp) await filter.locator('[data-testid="table-query-size-op"]').selectOption(sizeOp);
    const valueBox = filter.locator('[data-testid="table-query-value"]');
    if (value !== undefined && (await valueBox.isVisible())) await valueBox.fill(String(value));
    await expect(page.locator("#table-query-dql")).toHaveText(dql);
    await page.locator("#table-query-run").click();
    await expect(page.locator("#rows-table tbody tr")).toHaveCount(count);
    for (const text of has || []) await expect(page.locator("#rows-table tbody")).toContainText(text);
    for (const text of missing || []) await expect(page.locator("#rows-table tbody")).not.toContainText(text);
  };

  await runFilter({
    attr: "title",
    op: "begins_with",
    value: "Blue",
    dql: `SCAN * FROM ${name} WHERE begins_with(title, 'Blue')`,
    count: 1,
    has: ["Blue mug"],
    missing: ["Arc lamp"],
  });
  await runFilter({
    attr: "name",
    op: "contains",
    value: "hello",
    dql: `SCAN * FROM ${name} WHERE contains(name, 'hello')`,
    count: 1,
    has: ["hello world"],
  });
  await runFilter({
    attr: "tags",
    op: "contains",
    value: "metal",
    dql: `SCAN * FROM ${name} WHERE contains(tags, 'metal')`,
    count: 1,
    has: ["Arc lamp"],
  });
  await runFilter({
    attr: "tags",
    op: "exists",
    dql: `SCAN * FROM ${name} WHERE attribute_exists(tags)`,
    count: 3,
    has: ["Blue mug", "Arc lamp", "USB-C cable"],
    missing: ["Field notes"],
  });
  await runFilter({
    attr: "tags",
    op: "not_exists",
    dql: `SCAN * FROM ${name} WHERE attribute_not_exists(tags)`,
    count: 1,
    has: ["Field notes"],
  });
  await runFilter({
    attr: "qty",
    op: "attribute_type",
    type: "N",
    dql: `SCAN * FROM ${name} WHERE attribute_type(qty, 'N')`,
    count: 3,
    has: ["Blue mug", "USB-C cable"],
    missing: ["Field notes"],
  });
  await runFilter({
    attr: "tags",
    op: "size",
    sizeOp: "gt",
    value: "2",
    dql: `SCAN * FROM ${name} WHERE size(tags) > 2`,
    count: 1,
    has: ["Arc lamp"],
  });
  await page.screenshot({ path: playwrightArtifact(`table-query-functions-${testInfo.project.name}.png`) });

  await filter.locator('[data-testid="table-query-remove"]').click();
  await page.locator("#table-query-query").click();
  await page.locator('[data-testid="table-query-key-row"] [data-testid="table-query-value"]').fill("store-1");
  await page.locator("#table-query-add-key").click();
  const range = page.locator('[data-testid="table-query-key-row"]').nth(1);
  await range.locator('[data-testid="table-query-op"]').selectOption("begins_with");
  await range.locator('[data-testid="table-query-value"]').fill("ITEM#");
  await expect(page.locator("#table-query-dql")).toHaveText(
    `SELECT * FROM ${name} WHERE id = 'store-1' AND begins_with(sk, 'ITEM#')`,
  );
  await page.locator("#table-query-run").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(2);
  await expect(page.locator("#rows-table tbody")).toContainText("Blue mug");
  await expect(page.locator("#rows-table tbody")).toContainText("Arc lamp");
  await expect(page.locator("#rows-table tbody")).not.toContainText("Field notes");
});

test("table browser keys mode batch-gets typed hash and hash-range keys", async ({ page, app }, testInfo) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY, sk NUMBER RANGE KEY);`,
      `INSERT INTO ${name} (id, sk, status) VALUES ('ada', 1, 'open'), ('ada', 2, 'shut'), ('tom', 3, 'open');`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "3 rows inserted" })).toBeVisible();

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(3);

  const scanLead = await modeGroupBox(page);
  await page.locator("#table-query-keys-mode").click();
  await expect(page.locator("#table-query-keys-mode")).toHaveAttribute("aria-pressed", "true");
  const keysLead = await modeGroupBox(page);
  expect(keysLead.left).toBe(scanLead.left);
  expect(keysLead.top).toBe(scanLead.top);
  expect(Math.abs(keysLead.sourceLeft - scanLead.sourceLeft)).toBeLessThanOrEqual(1);
  expect(keysLead.sourceWidth).toBe(scanLead.sourceWidth);
  await expect(page.locator("#table-query-source-wrap")).toBeVisible();
  await expect(page.locator("#table-query-order")).toBeHidden();
  await expect(page.locator("#table-query-key-section")).toBeHidden();
  await expect(page.locator("#table-query-filter-section")).toBeHidden();
  await expect(page.locator("#table-query-batch")).toBeVisible();
  await expect(page.locator("#table-query-source")).toHaveValue("");
  await expect(page.locator("#table-query-source")).toContainText("Table");
  await expect(page.locator("#table-query-run")).toBeDisabled();
  await expect(page.locator("#table-query-error")).toHaveText("Keys needs at least one key.");

  await page.locator('[data-testid="table-query-batch-keys"]').fill("(ada, 1)\n(tom, 3)");
  await expect(page.locator("#table-query-dql")).toHaveText(
    `SELECT * FROM ${name} KEYS IN ('ada', 1), ('tom', 3)`,
  );
  await page.locator('[data-testid="table-query-batch-keys"]').fill("((ada, 1), (tom, 3),)");
  await expect(page.locator("#table-query-dql")).toHaveText(
    `SELECT * FROM ${name} KEYS IN ('ada', 1), ('tom', 3)`,
  );
  await page.locator("#table-query-run").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(2);
  await expect(page.locator("#rows-table tbody")).toContainText("ada");
  await expect(page.locator("#rows-table tbody")).toContainText("tom");
  await expect(page.locator("#rows-table tbody")).not.toContainText("shut");
  await page.screenshot({ path: playwrightArtifact(`table-query-keys-mode-${testInfo.project.name}.png`) });
});

test("table browser keys mode queries a chosen index", async ({ page, app }, testInfo) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY, sk NUMBER RANGE KEY) GLOBAL INDEX ('by-status', status STRING, sk NUMBER);`,
      `INSERT INTO ${name} (id, sk, status) VALUES ('ada', 1, 'open'), ('ada', 2, 'shut'), ('tom', 3, 'open');`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "3 rows inserted" })).toBeVisible();

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(3);

  await page.locator("#table-query-keys-mode").click();
  await expect(page.locator("#table-query-source")).toHaveValue("");
  const tableLead = await modeGroupBox(page);
  await page.locator("#table-query-source").selectOption("by-status");
  const indexLead = await modeGroupBox(page);
  expect(Math.abs(indexLead.sourceLeft - tableLead.sourceLeft)).toBeLessThanOrEqual(1);
  expect(indexLead.sourceWidth).toBe(tableLead.sourceWidth);
  expect(indexLead.sourceWidth).toBe(380);
  await page.locator('[data-testid="table-query-batch-keys"]').fill("open");
  await expect(page.locator("#table-query-dql")).toHaveText(
    `SELECT * FROM ${name} WHERE status = 'open' USING by-status`,
  );
  await page.locator("#table-query-run").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(2);
  await expect(page.locator("#rows-table tbody")).toContainText("ada");
  await expect(page.locator("#rows-table tbody")).toContainText("tom");
  await expect(page.locator("#rows-table tbody")).not.toContainText("shut");
  await page.screenshot({ path: playwrightArtifact(`table-query-keys-index-${testInfo.project.name}.png`) });
});

test("table browser keys mode batch-gets a list of hash keys", async ({ page, app }, testInfo) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY);`,
      `INSERT INTO ${name} (id, status) VALUES ('ada', 'open'), ('tom', 'open'), ('sue', 'shut');`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "3 rows inserted" })).toBeVisible();

  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(3);

  await page.locator("#table-query-keys-mode").click();
  await page.locator('[data-testid="table-query-batch-keys"]').fill("ada, tom");
  await expect(page.locator("#table-query-dql")).toHaveText(`SELECT * FROM ${name} KEYS IN 'ada', 'tom'`);
  // A tuple is a (hash, range) pair, so three values in parentheses is not a key.
  await page.locator('[data-testid="table-query-batch-keys"]').fill("(ada, tom, sue)");
  await expect(page.locator("#table-query-error")).toHaveText("Line 1: a key has at most two parts: (hash, range).");
  await page.locator('[data-testid="table-query-batch-keys"]').fill("ada, tom, sue");
  await expect(page.locator("#table-query-dql")).toHaveText(`SELECT * FROM ${name} KEYS IN 'ada', 'tom', 'sue'`);
  await page.locator("#table-query-run").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(3);
  await expect(page.locator("#rows-table tbody")).toContainText("ada");
  await expect(page.locator("#rows-table tbody")).toContainText("tom");
  await expect(page.locator("#rows-table tbody")).toContainText("sue");
  await page.screenshot({ path: playwrightArtifact(`table-query-keys-hash-list-${testInfo.project.name}.png`) });
});

test("names insert, update, delete, count, and drop results", async ({ page, app }) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY, n NUMBER);`,
      // The server keeps comment lines in the statement text; labels still name the operation.
      "-- seed two rows",
      `INSERT INTO ${name} (id, n) VALUES ('a', 1), ('b', 2);`,
      "-- bump one",
      "-- and keep the other",
      `UPDATE ${name} SET n = 9 WHERE id = 'a';`,
      `DELETE FROM ${name} WHERE id = 'b';`,
      `SCAN count(*) FROM ${name};`,
      "-- clean up",
      `DROP TABLE ${name};`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await openResultPane(page, "history");
  const notes = page.locator('[data-testid="result-note"]');
  await expect(notes.filter({ hasText: "2 rows inserted" })).toHaveCount(1);
  await expect(notes.filter({ hasText: "one row updated" })).toHaveCount(1);
  await expect(notes.filter({ hasText: "one row deleted" })).toHaveCount(1);
  await expect(notes.filter({ hasText: /^one row$/ })).toHaveCount(1);
  await expect(notes.filter({ hasText: "one table dropped" })).toHaveCount(1);
  await expect(notes.filter({ hasText: "did not exist" })).toHaveCount(1);
});

function trackRowPages(page, options = {}) {
  const pages = [];
  const needle = options.dql || "";
  page.on("request", (request) => {
    const url = new URL(request.url());
    if (url.pathname !== "/api/rows") return;
    const dql = url.searchParams.get("dql") || "";
    if (needle && !dql.includes(needle)) return;
    pages.push(Number(url.searchParams.get("page") || "0"));
  });
  return pages;
}

async function columnValues(page, name) {
  return page.locator("#rows-table").evaluate((table, name) => {
    const headers = [...table.querySelectorAll("thead th")].map((cell) => {
      const label = cell.querySelector(".col-label span");
      return (label ? label.textContent : cell.textContent).trim();
    });
    const index = headers.indexOf(name);
    if (index < 0) return [];
    return [...table.querySelectorAll("tbody tr")].map((row) => row.children[index]?.textContent ?? "");
  }, name);
}

async function expectNumericPage(page, pageIndex, total = 1000) {
  const size = 50;
  const start = pageIndex * size + 1;
  const end = Math.min(start + size - 1, total);
  await expect(page.locator("#page-label")).toHaveText(`Showing ${start}–${end}`);
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(end - start + 1);
  await expect
    .poll(async () => (await columnValues(page, "id")).map(Number))
    .toEqual(Array.from({ length: end - start + 1 }, (_, index) => start + index));
  if (pageIndex === 0) await expect(page.locator("#page-prev")).toBeDisabled();
  else await expect(page.locator("#page-prev")).toBeEnabled();
  if (end >= total) await expect(page.locator("#page-next")).toBeDisabled();
  else await expect(page.locator("#page-next")).toBeEnabled();
}

async function modeGroupBox(page) {
  return page.locator(".table-query-lead").evaluate((node) => {
    const group = node.getBoundingClientRect();
    const buttons = node.querySelector("[aria-label='Read type']").getBoundingClientRect();
    const source = node.querySelector("#table-query-source").getBoundingClientRect();
    return {
      left: Math.round(group.left),
      top: Math.round(group.top),
      buttonsRight: Math.round(buttons.right),
      sourceLeft: Math.round(source.left),
      sourceWidth: Math.round(source.width),
    };
  });
}

function uniqueTable() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

async function edgeOffset(control) {
  return control.evaluate((button) => {
    const parent = button.closest("#rows-body, .result-card-body").getBoundingClientRect();
    return Math.round(button.getBoundingClientRect().left - parent.left);
  });
}

async function headerTops(page) {
  return page.locator("#rows-table thead th").evaluateAll((cells) =>
    cells.map((cell) => Math.round(cell.getBoundingClientRect().top)),
  );
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

test("table browser splits controls from the query as built and its plan", async ({ page, app }, testInfo) => {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY, sk NUMBER RANGE KEY);`,
      `INSERT INTO ${name} (id, sk) VALUES ('ada', 1), ('ada', 2), ('tom', 3);`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "3 rows inserted" })).toBeVisible();
  await page.evaluate(() => {
    localStorage.removeItem("dqlrs-web.table-query-split");
    localStorage.removeItem("dqlrs-web.table-query-side-hidden");
  });
  const explains = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname === "/api/explain") explains.push(request.url());
  });
  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(3);

  // Controls at the left, 60% by default; tabs for the query as built and its plan at the right.
  const form = await page.locator("#table-query").boundingBox();
  const controls = await page.locator("#table-query-controls").boundingBox();
  const side = await page.locator(".table-query-side").boundingBox();
  expect(Math.abs(controls.width / form.width - 0.6)).toBeLessThan(0.01);
  expect(side.x).toBeGreaterThanOrEqual(controls.x + controls.width - 1);
  await expect(page.locator(".table-query-side .result-tab")).toHaveText(["Query as built", "Explain"]);
  await expect(page.locator("#table-query-tab-dql")).toHaveAttribute("aria-selected", "true");
  await expect(page.locator("#table-query-dql")).toHaveText(`SCAN * FROM ${name}`);
  // The divider is the main side divider's component.
  await expect(page.locator("#table-query-resize")).toHaveClass(/side-resize/);
  await expect(page.locator("#table-query-resize .side-grip")).toHaveCount(1);
  await page.screenshot({ path: playwrightArtifact(`table-query-split-${testInfo.project.name}.png`) });

  // No plan is fetched while the Explain tab is not showing.
  await page.locator("#table-query-query").click();
  await page.locator("#table-query-scan").click();
  await page.waitForTimeout(500);
  expect(explains).toEqual([]);

  // Explain shows the plan of what Run would read, and follows the controls.
  await page.locator("#table-query-tab-explain").click();
  await expect(page.locator("#table-query-tab-explain")).toHaveAttribute("aria-selected", "true");
  await expect(page.locator("#table-query-dql")).toBeHidden();
  const explain = page.locator("#table-query-explain");
  await expect(explain.locator('[data-testid="explain-op"]').first()).toHaveText(/scan/i);
  await page.locator("#table-query-query").click();
  // A query without a hash key value has no plan; Explain says what is missing.
  await expect(explain).toContainText("Query needs a hash key value.");
  await page.locator('[data-testid="table-query-key-row"]').first().locator('[data-testid="table-query-value"]').fill("ada");
  await expect(explain.locator('[data-testid="explain-op"]').first()).toHaveText(/query/i);
  // The pane's plan is plain steps: no "Explain" title or JSON box inside it. The JSON box sits
  // on the tab bar and only shows with Explain selected.
  await expect(explain.locator(".explain-head")).toHaveCount(0);
  const json = page.locator('[data-testid="table-query-json"]');
  await expect(json).toBeVisible();
  await expect(explain.locator('[data-testid="explain-json"]')).toBeHidden();
  await page.screenshot({ path: playwrightArtifact(`table-query-explain-${testInfo.project.name}.png`) });
  await json.check();
  await expect(explain.locator('[data-testid="explain-json"]')).toBeVisible();
  await expect(explain.locator('[data-testid="explain-step"]').first()).toBeHidden();
  await json.uncheck();
  await expect(explain.locator('[data-testid="explain-step"]').first()).toBeVisible();

  // The toggle at the right end hides the pane; with it hidden, no plan is fetched.
  const toggle = page.locator("#table-query-side-toggle");
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  const toggleBox = await toggle.boundingBox();
  expect(Math.abs(form.x + form.width - (toggleBox.x + toggleBox.width))).toBeLessThan(16);
  await toggle.click();
  await expect(page.locator(".table-query-side")).toBeHidden();
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await expect(toggle).toBeVisible();
  expect((await page.locator("#table-query-controls").boundingBox()).width).toBeGreaterThan(form.width - 2);
  await page.screenshot({ path: playwrightArtifact(`table-query-side-hidden-${testInfo.project.name}.png`) });
  const before = explains.length;
  await page.locator("#table-query-scan").click();
  await page.waitForTimeout(500);
  expect(explains.length).toBe(before);
  await toggle.click();
  await expect(page.locator(".table-query-side")).toBeVisible();
  // Showing the pane again with Explain open fetches a fresh plan.
  await expect.poll(() => explains.length).toBeGreaterThan(before);
  await expect(explain.locator('[data-testid="explain-op"]').first()).toBeVisible();

  await page.locator("#table-query-tab-dql").click();
  await expect(page.locator("#table-query-dql")).toHaveText(/^SCAN \* FROM /);
  await expect(page.locator('[data-testid="table-query-json"]')).toBeHidden();
  await page.locator("#table-query-query").click();
  await expect(page.locator("#table-query-dql")).toHaveText(/^SELECT \* FROM .* WHERE id = 'ada'/);
  await page.locator("#table-query-scan").click();

  // Dragging the divider resizes the panes and the width is kept across reloads.
  const handle = await page.locator("#table-query-resize").boundingBox();
  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
  await page.mouse.down();
  await page.mouse.move(handle.x + handle.width / 2 - 150, handle.y + handle.height / 2, { steps: 5 });
  await page.mouse.up();
  const narrower = await page.locator("#table-query-controls").boundingBox();
  expect(narrower.width).toBeLessThan(controls.width - 100);
  const saved = await page.evaluate(() => Number(localStorage.getItem("dqlrs-web.table-query-split")));
  expect(Math.abs(saved - narrower.width / form.width)).toBeLessThan(0.01);
  // The right pane stops at 55% of the panel, however far the divider is dragged.
  const left = await page.locator("#table-query-resize").boundingBox();
  await page.mouse.move(left.x + left.width / 2, left.y + left.height / 2);
  await page.mouse.down();
  await page.mouse.move(form.x + 20, left.y + left.height / 2, { steps: 8 });
  await page.mouse.up();
  const widest = await page.locator(".table-query-side").boundingBox();
  expect(widest.width / form.width).toBeLessThanOrEqual(0.551);
  expect(widest.width / form.width).toBeGreaterThan(0.53);
  await page.locator("#table-query-resize").dblclick();
  const reset = await page.locator("#table-query-controls").boundingBox();
  expect(Math.abs(reset.width / form.width - 0.6)).toBeLessThan(0.01);

  // Dragging the right pane under 10% of the panel hides it; dragging back shows it again.
  const grip = await page.locator("#table-query-resize").boundingBox();
  await page.mouse.move(grip.x + grip.width / 2, grip.y + grip.height / 2);
  await page.mouse.down();
  await page.mouse.move(form.x + form.width * 0.95, grip.y + grip.height / 2, { steps: 8 });
  await page.mouse.up();
  await expect(page.locator(".table-query-side")).toBeHidden();
  await expect(page.locator("#table-query-side-toggle")).toHaveAttribute("aria-pressed", "false");
  expect(await page.evaluate(() => localStorage.getItem("dqlrs-web.table-query-side-hidden"))).toBe("1");
  await page.reload();
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await page.locator(`#table-${name}`).click();
  await expect(page.locator("#table-query")).toBeVisible();
  await expect(page.locator(".table-query-side")).toBeHidden();
  await page.locator("#table-query-side-toggle").click();
  await expect(page.locator(".table-query-side")).toBeVisible();

  // The bottom edge resizes the panel's height (the results pane's divider), kept across reloads.
  await page.evaluate(() => localStorage.removeItem("dqlrs-web.table-query-height"));
  const panel = page.locator("#table-query");
  const natural = await panel.boundingBox();
  await expect(page.locator("#table-query-height .results-grip")).toHaveCount(1);
  const edge = await page.locator("#table-query-height").boundingBox();
  await page.mouse.move(edge.x + edge.width / 2, edge.y + edge.height / 2);
  await page.mouse.down();
  await page.mouse.move(edge.x + edge.width / 2, edge.y + edge.height / 2 + 120, { steps: 6 });
  await page.mouse.up();
  const taller = await panel.boundingBox();
  expect(taller.height).toBeGreaterThan(natural.height + 100);
  expect(Number(await page.evaluate(() => localStorage.getItem("dqlrs-web.table-query-height")))).toBe(Math.round(taller.height));
  await page.screenshot({ path: playwrightArtifact(`table-query-taller-${testInfo.project.name}.png`) });
  // Shorter than its controls, the panel scrolls them; Run stays reachable.
  await page.locator("#table-query-height").focus();
  for (let i = 0; i < 12; i += 1) await page.keyboard.press("ArrowUp");
  const short = await panel.boundingBox();
  expect(short.height).toBeLessThan(natural.height);
  await page.locator("#table-query-run").scrollIntoViewIfNeeded();
  await expect(page.locator("#table-query-run")).toBeInViewport();
  await page.locator("#table-query-height").dblclick();
  expect(Math.abs((await panel.boundingBox()).height - natural.height)).toBeLessThan(2);
  expect(await page.evaluate(() => localStorage.getItem("dqlrs-web.table-query-height"))).toBeNull();
});
