import { test, expect } from "../support/app.js";
import { lineByText, lineOverlaps, node, openApp, replaceEditor } from "../support/helpers.js";

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

test("resizes the results pane from the top handle", async ({ page }) => {
  const results = page.locator("#results");
  const handle = page.locator("#results-resize");
  await expect(handle).toBeVisible();
  const before = await results.boundingBox();
  const grip = await handle.boundingBox();
  const x = grip.x + grip.width / 2;
  await page.mouse.move(x, grip.y + grip.height / 2);
  await page.mouse.down();
  await page.mouse.move(x, grip.y - 90, { steps: 8 });
  await page.mouse.up();
  const taller = await results.boundingBox();
  expect(taller.height).toBeGreaterThan(before.height + 60);
  const stored = Number(await page.evaluate(() => localStorage.getItem("dqlrs-web.results-height")));
  expect(Math.abs(stored - taller.height)).toBeLessThan(3);

  await page.reload();
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  const restored = await page.locator("#results").boundingBox();
  expect(Math.abs(restored.height - taller.height)).toBeLessThan(3);

  await handle.focus();
  await page.keyboard.press("ArrowDown");
  const shorter = await page.locator("#results").boundingBox();
  expect(shorter.height).toBeLessThan(restored.height - 10);

  const edge = await handle.boundingBox();
  await page.mouse.move(edge.x + edge.width / 2, edge.y + 4);
  await page.mouse.down();
  await page.mouse.move(edge.x + edge.width / 2, 0, { steps: 6 });
  await page.mouse.up();
  const editor = await page.locator(".editor").boundingBox();
  expect(editor.height).toBeGreaterThan(140);

  await handle.dblclick();
  await expect.poll(() => page.evaluate(() => localStorage.getItem("dqlrs-web.results-height"))).toBeNull();
  const reset = await page.locator("#results").boundingBox();
  const view = await page.locator("#query-view").boundingBox();
  expect(Math.abs(reset.height - view.height * 0.46)).toBeLessThan(8);
});

test("shows the endpoint and the editor shortcut", async ({ page }) => {
  await expect(page.locator("#endpoint")).toHaveText("localhost:8000");
  await expect(page.locator("#run-shortcut")).toHaveText("Ctrl+Enter");
  await expect(page.locator("#run")).toHaveAttribute("aria-keyshortcuts", "Control+Enter");
});

test("pads one line above the editor and five lines below it", async ({ page }) => {
  const ratio = await page.evaluate(() => {
    const content = document.querySelector(".cm-content");
    const line = document.querySelector(".cm-line");
    const style = getComputedStyle(content);
    const lineHeight = line.getBoundingClientRect().height;
    return {
      top: parseFloat(style.paddingTop) / lineHeight,
      bottom: parseFloat(style.paddingBottom) / lineHeight,
    };
  });
  expect(ratio.top).toBeGreaterThan(0.85);
  expect(ratio.top).toBeLessThan(1.2);
  expect(ratio.bottom).toBeGreaterThan(4.7);
  expect(ratio.bottom).toBeLessThan(5.3);
});

test("colors SELECT and outlines the current query without covering the next one", async ({ page }) => {
  await node(page, "queries/read.dql").click();
  await expect(page.locator("#file-name")).toHaveText("read.dql");
  await expect(page.locator(".cm-run-fill")).toHaveCount(0);

  await page.locator("#run").click();
  await expect(page.locator("#result-meta")).toHaveText("Nothing to run");

  const color = await page.evaluate(() => {
    const span = [...document.querySelectorAll(".cm-content span")].find((el) => el.textContent === "SELECT");
    return span ? getComputedStyle(span).color : "";
  });
  expect(color).toBe("rgb(15, 118, 110)");

  await page.locator(".cm-line", { hasText: "WHERE id = 'a'" }).click();
  await expect(page.locator(".cm-run-fill")).toBeVisible();
  const cursor = await lineOverlaps(page);
  expect(lineByText(cursor, "-- header stays with the select").overlap).toBeGreaterThan(
    lineByText(cursor, "-- header stays with the select").height * 0.8,
  );
  expect(lineByText(cursor, "SELECT * FROM pw_missing_table").overlap).toBeGreaterThan(
    lineByText(cursor, "SELECT * FROM pw_missing_table").height * 0.8,
  );
  expect(lineByText(cursor, "WHERE id = 'a'").overlap).toBeGreaterThan(
    lineByText(cursor, "WHERE id = 'a'").height * 0.8,
  );
  const nextHeader = lineByText(cursor, "-- next");
  expect(nextHeader.overlap).toBeGreaterThan(2);
  expect(nextHeader.overlap).toBeLessThan(12);
  expect(lineByText(cursor, "SCAN * FROM pw_missing_table").overlap).toBeLessThan(2);

  await page.locator(".cm-line", { hasText: "SELECT * FROM pw_missing_table" }).dragTo(
    page.locator(".cm-line", { hasText: "WHERE id = 'a'" }),
  );
  const selected = await lineOverlaps(page);
  expect(lineByText(selected, "SELECT * FROM pw_missing_table").overlap).toBeGreaterThan(
    lineByText(selected, "SELECT * FROM pw_missing_table").height * 0.8,
  );
  expect(lineByText(selected, "WHERE id = 'a'").overlap).toBeGreaterThan(
    lineByText(selected, "WHERE id = 'a'").height * 0.8,
  );
  expect(lineByText(selected, "-- header stays with the select").overlap).toBeLessThan(12);
  expect(lineByText(selected, "-- next").overlap).toBeLessThan(2);
  await expect(page.locator(".cm-run-fill")).toHaveCount(1);
});

test("frames a partial line selection as a whole line and highlights the text", async ({ page }) => {
  await node(page, "queries/read.dql").click();
  const line = page.locator(".cm-line", { hasText: "SELECT * FROM pw_missing_table" });
  const box = await line.boundingBox();
  await page.mouse.move(box.x + 14, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + 52, box.y + box.height / 2, { steps: 4 });
  await page.mouse.up();

  await expect(page.locator(".cm-selectionBackground").first()).toBeVisible();
  await expect(page.locator(".cm-run-fill")).toBeVisible();
  const metrics = await page.evaluate(() => {
    const fill = document.querySelector(".cm-run-fill").getBoundingClientRect();
    const selected = [...document.querySelectorAll(".cm-selectionBackground")].map((el) => el.getBoundingClientRect());
    const selectLeft = Math.min(...selected.map((rect) => rect.left));
    const selectRight = Math.max(...selected.map((rect) => rect.right));
    const row = [...document.querySelectorAll(".cm-content .cm-line")].find((el) =>
      el.textContent.includes("SELECT * FROM pw_missing_table"),
    );
    const range = document.createRange();
    range.selectNodeContents(row);
    const text = range.getBoundingClientRect();
    const keyword = [...row.querySelectorAll("span")].find((el) => el.textContent === "SELECT");
    return {
      fillLeft: fill.left,
      fillRight: fill.right,
      textLeft: text.left,
      textRight: text.right,
      selectWidth: selectRight - selectLeft,
      textWidth: text.width,
      keyword: keyword ? getComputedStyle(keyword).color : "",
      highlight: getComputedStyle(document.querySelector(".cm-selectionBackground")).backgroundColor,
    };
  });
  expect(metrics.fillLeft).toBeLessThanOrEqual(metrics.textLeft + 1);
  expect(metrics.fillRight).toBeGreaterThanOrEqual(metrics.textRight - 1);
  expect(metrics.selectWidth).toBeGreaterThan(4);
  expect(metrics.selectWidth).toBeLessThan(metrics.textWidth * 0.75);
  expect(metrics.keyword).toBe("rgb(15, 118, 110)");
  expect(metrics.highlight).not.toBe("rgba(0, 0, 0, 0)");
  const layerOrder = await page.evaluate(() => {
    const z = (selector) => Number(getComputedStyle(document.querySelector(selector)).zIndex);
    return { selection: z(".cm-selectionLayer"), band: z(".cm-band-layer"), window: z(".cm-run-fill-layer") };
  });
  expect(layerOrder.selection).toBeGreaterThan(layerOrder.band);
  expect(layerOrder.window).toBeGreaterThan(layerOrder.selection);

  const framed = await lineOverlaps(page);
  expect(lineByText(framed, "SELECT * FROM pw_missing_table").overlap).toBeGreaterThan(
    lineByText(framed, "SELECT * FROM pw_missing_table").height * 0.8,
  );
  expect(lineByText(framed, "WHERE id = 'a'").overlap).toBeLessThan(12);
  expect(lineByText(framed, "-- header stays with the select").overlap).toBeLessThan(12);
});

test("saves highlight toggles and switches write bands to a gutter bar", async ({ page }) => {
  await node(page, "queries/read.dql").click();
  await expect(page.locator(".cm-band-alt").first()).toBeVisible();
  await expect(page.locator(".cm-band-write-even").first()).toBeVisible();

  await page.locator("#opt-even-odd").uncheck();
  await expect(page.locator(".cm-band-alt")).toHaveCount(0);
  await expect(page.locator(".cm-band-write-even")).toHaveCount(0);
  await expect(page.locator(".cm-band-write").first()).toBeVisible();

  await page.locator("#opt-minimal-write").check();
  await expect.poll(() => visibleWriteBars(page)).toBeGreaterThan(0);
  await expect(page.locator(".cm-band-write")).toHaveCount(0);

  const stored = await page.evaluate(() => JSON.parse(localStorage.getItem("dqlrs-web.highlight")));
  expect(stored).toEqual({ minimalWrite: true, evenOdd: false });

  await page.reload();
  await expect(page.locator("#opt-minimal-write")).toBeChecked();
  await expect(page.locator("#opt-even-odd")).not.toBeChecked();
  await node(page, "queries/read.dql").click();
  await expect.poll(() => visibleWriteBars(page)).toBeGreaterThan(0);
  await expect(page.locator(".cm-band-alt")).toHaveCount(0);
});

async function visibleWriteBars(page) {
  return page.locator(".cm-write-bar").evaluateAll((elements) =>
    elements.filter((element) => {
      const rect = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return rect.height > 2 && rect.width > 1 && style.visibility !== "hidden";
    }).length,
  );
}

test("runs the query at the cursor with Ctrl+Enter and marks a failure", async ({ page }) => {
  await node(page, "queries/read.dql").click();
  await page.locator(".cm-line", { hasText: "SELECT * FROM pw_missing_table" }).click();
  await page.keyboard.press("Control+Enter");
  const error = page.locator('[data-testid="result-note"].error');
  await expect(error).toBeVisible();
  await expect(error).not.toHaveText("");
  await expect(page.locator('[data-testid="run-status-error"]')).toBeVisible();
  await expect(page.locator("#run-progress")).toBeHidden();
  await expect(page.locator("#run")).toBeEnabled();
});

test("runs a selection and shows the result table", async ({ page, app }) => {
  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY);`,
      `INSERT INTO ${name} (id, label) VALUES ('a', 'alpha');`,
      `SELECT * FROM ${name} WHERE id = 'a';`,
    ].join("\n"),
  );
  await page.keyboard.press("Control+A");
  await page.locator("#run").click();
  await expect(page.locator("#result-table")).toContainText("alpha");
  await expect(page.locator("#result-meta")).toHaveText("1 row");
  await expect(page.locator('[data-testid="run-status-ok"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator("#run-progress")).toBeHidden();
});

test("shows an explain plan as operations and conditions", async ({ page, app }) => {
  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY, n NUMBER RANGE KEY);`,
    ].join("\n"),
  );
  await page.keyboard.press("Control+A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);

  await replaceEditor(page, `EXPLAIN SELECT * FROM ${name} WHERE id = 'a' AND n > 1;`);
  await page.keyboard.press("Control+A");
  await page.locator("#run").click();

  const plan = page.locator('[data-testid="explain-plan"]');
  await expect(plan).toBeVisible();
  await expect(plan.locator('[data-testid="explain-step"]')).toHaveCount(1);
  await expect(plan.locator('[data-testid="explain-op"]')).toHaveText("query");
  await expect(plan.locator('[data-testid="explain-target"]')).toHaveText(name);
  const fields = plan.locator(".explain-fields");
  const field = (label) =>
    fields.locator("dt", { hasText: new RegExp("^" + label + "$") }).locator("xpath=following-sibling::dd[1]");
  await expect(field("key condition")).toContainText("n > ");
  await expect(field("filter")).toContainText("id = ");
  await expect(field("index")).toHaveText("TABLE");
  await expect(page.locator("#result-meta")).toHaveText("1 step");
  await expect(page.locator('[data-testid="result-note"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="run-status-error"]')).toHaveCount(0);

  const toggle = plan.locator('[data-testid="explain-raw"]');
  const raw = plan.locator('[data-testid="explain-json"]');
  await expect(toggle).not.toBeChecked();
  await expect(raw).toBeHidden();
  await toggle.check();
  await expect(plan.locator('[data-testid="explain-step"]')).toBeHidden();
  await expect(raw).toBeVisible();
  const text = await raw.innerText();
  expect(text).toContain('\n    "operation": "query"');
  const parsed = JSON.parse(text);
  expect(parsed).toHaveLength(1);
  expect(parsed[0].operation).toBe("query");
  expect(parsed[0].target).toBe(name);
  expect(parsed[0].index).toBe("TABLE");
  expect(parsed[0].key_condition).toContain("n > ");
  expect(parsed[0].filter).toContain("id = ");
  await page.reload();
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  await replaceEditor(page, `EXPLAIN SELECT * FROM ${name} WHERE id = 'a' AND n > 1;`);
  await page.keyboard.press("Control+A");
  await page.locator("#run").click();
  const again = page.locator('[data-testid="explain-plan"]');
  await expect(again.locator('[data-testid="explain-raw"]')).toBeChecked();
  await expect(again.locator('[data-testid="explain-json"]')).toBeVisible();
  await again.locator('[data-testid="explain-raw"]').uncheck();
  await expect(again.locator('[data-testid="explain-step"]')).toBeVisible();
  await expect(again.locator('[data-testid="explain-json"]')).toBeHidden();
});

test("shows a progress bar and a running gutter mark for a throttled insert", async ({ page, app }) => {
  const name = uniqueTable();
  app.trackTable(name);
  const tuples = Array.from({ length: 30 }, (_, index) => `('r${index}', ${index})`).join(", ");
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY);`,
      `INSERT INTO ${name} (id, n) VALUES ${tuples} THROTTLE 10 10;`,
    ].join("\n"),
  );
  await page.keyboard.press("Control+A");
  await page.locator("#run").click();
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator("#run-progress")).toBeVisible();
  await expect(page.locator('[data-testid="run-status-running"]')).toBeVisible();
  await expect(page.locator("#run-progress")).toBeHidden({ timeout: 30_000 });
  await expect(page.locator("#run")).toBeEnabled();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "30 affected" })).toBeVisible();
  await expect(page.locator('[data-testid="run-status-error"]')).toHaveCount(0);
});

test("places a play button on the left of the run box and runs that range", async ({ page, app }) => {
  await node(page, "queries/read.dql").click();
  await expect(page.locator(".run-float")).toHaveCount(0);

  await page.locator(".cm-line", { hasText: "SELECT * FROM pw_missing_table" }).click();
  const button = page.locator(".run-float");
  await expect(button).toBeVisible();
  await expect(button).toHaveAttribute("aria-label", "Run selection");
  const place = await page.evaluate(() => {
    const buttonBox = document.querySelector(".run-float").getBoundingClientRect();
    const fill = document.querySelector(".cm-run-fill").getBoundingClientRect();
    const radius = parseFloat(getComputedStyle(document.querySelector(".run-float")).borderRadius);
    return {
      buttonRight: buttonBox.right,
      fillLeft: fill.left,
      buttonMidY: buttonBox.top + buttonBox.height / 2,
      fillTop: fill.top,
      fillBottom: fill.bottom,
      width: buttonBox.width,
      height: buttonBox.height,
      radius,
    };
  });
  expect(place.buttonRight).toBeLessThanOrEqual(place.fillLeft + 1);
  expect(place.buttonMidY).toBeGreaterThan(place.fillTop - 2);
  expect(place.buttonMidY).toBeLessThan(place.fillBottom + 2);
  expect(Math.abs(place.width - place.height)).toBeLessThan(1);
  expect(place.radius).toBeGreaterThanOrEqual(place.width / 2 - 0.5);

  const name = uniqueTable();
  app.trackTable(name);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${name};`,
      `CREATE TABLE ${name} (id STRING HASH KEY);`,
      `INSERT INTO ${name} (id, label) VALUES ('a', 'alpha');`,
      `SELECT * FROM ${name} WHERE id = 'a';`,
      `SCAN * FROM pw_missing_table;`,
    ].join("\n"),
  );
  await page.locator(".cm-line", { hasText: "DROP TABLE" }).dragTo(page.locator(".cm-line", { hasText: "VALUES" }));
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "1 affected" })).toBeVisible();

  await page.locator(".cm-line", { hasText: "INSERT INTO" }).dragTo(page.locator(".cm-line", { hasText: "SELECT * FROM" }));
  await page.locator(".run-float").click();
  await expect(page.locator("#result-table")).toContainText("alpha");
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator("#run")).toBeEnabled();
});

function uniqueTable() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}
