import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { themeColor } from "../support/helpers.js";

// Key inputs check the value against the key's type for every table. A string key takes any
// text; a number key says so before Run when the text is not a number.

function uniqueTable(kind) {
  return `pw_key_types_${kind}_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;
}

async function openTable(page, app, name, rows = 2) {
  await page.goto(`${app.baseURL}/`);
  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
  }, name);
  await page.locator("#mode-tables").click();
  await expect(page.locator(`#table-${name}`)).toBeVisible();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(rows);
}

test("number keys reject text in Keys and Query mode, string keys take it", async ({ page, app }, testInfo) => {
  const numeric = uniqueTable("num");
  const text = uniqueTable("str");
  app.trackTable(numeric);
  app.trackTable(text);
  app.command(`CREATE TABLE ${numeric} (n NUMBER HASH KEY, sk NUMBER RANGE KEY);`);
  app.command(`INSERT INTO ${numeric} (n, sk) VALUES (1, 10), (2, 20);`);
  app.command(`CREATE TABLE ${text} (id STRING HASH KEY, sk STRING RANGE KEY);`);
  app.command(`INSERT INTO ${text} (id, sk) VALUES ('a', 'x'), ('a', 'z'), ('b', 'y');`);

  const error = page.locator("#table-query-error");
  const run = page.locator("#table-query-run");
  const keys = page.locator('[data-testid="table-query-batch-keys"]');

  await openTable(page, app, numeric);
  await page.locator("#table-query-keys-mode").click();
  await keys.fill("(ada, 10)");
  await expect(error).toHaveText('n is a number. "ada" is not.');
  await expect(run).toBeDisabled();
  await keys.fill("(1, x)");
  await expect(error).toHaveText('sk is a number. "x" is not.');
  await page.screenshot({ path: playwrightArtifact(`table-key-types-keys-${testInfo.project.name}.png`) });
  await keys.fill("(1, 10), (2, 20)");
  await expect(error).toHaveText("");
  await expect(run).toBeEnabled();

  await page.locator("#table-query-query").click();
  const hashValue = page.locator('[data-testid="table-query-key-row"] [data-testid="table-query-value"]').first();
  await hashValue.fill("abc");
  await expect(error).toHaveText('n is a number. "abc" is not.');
  await expect(run).toBeDisabled();
  await page.screenshot({ path: playwrightArtifact(`table-key-types-query-${testInfo.project.name}.png`) });
  await hashValue.fill("2");
  await expect(error).toHaveText("");
  await run.click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(1);

  await openTable(page, app, text, 3);
  await page.locator("#table-query-keys-mode").click();
  await keys.fill("(a, x), (b, 1)");
  await expect(error).toHaveText("");
  await expect(run).toBeEnabled();
});

test("a,b,c reads every item under each hash key on a table with a range key", async ({ page, app }, testInfo) => {
  const name = uniqueTable("range");
  app.trackTable(name);
  app.command(`CREATE TABLE ${name} (id STRING HASH KEY, sk STRING RANGE KEY);`);
  app.command(`INSERT INTO ${name} (id, sk) VALUES ('a', 'x'), ('a', 'z'), ('b', 'y'), ('c', 'w'), ('d', 'v');`);
  await openTable(page, app, name, 5);

  const error = page.locator("#table-query-error");
  const keys = page.locator('[data-testid="table-query-batch-keys"]');
  await page.locator("#table-query-keys-mode").click();
  await keys.fill("a,b,c");
  await expect(error).toHaveText("");
  await expect(page.locator("#table-query-dql")).toHaveText(`SELECT * FROM ${name} KEYS IN 'a', 'b', 'c'`);
  await page.locator("#table-query-run").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(4);
  await expect(page.locator("#rows-table tbody")).not.toContainText("d");
  await page.screenshot({ path: playwrightArtifact(`table-key-types-hash-list-${testInfo.project.name}.png`) });

  // Repeated keys are left out of the statement, and the hint says how many.
  await keys.fill("a, b, a, (b, y), c, c");
  await expect(error).toHaveText("");
  await expect(page.locator("#table-query-dql")).toHaveText(`SELECT * FROM ${name} KEYS IN 'a', 'b', 'c'`);
  await expect(page.locator("#table-query-batch-hint")).toHaveText("id keys, or (id, sk) pairs · 3 repeated keys left out");
  await page.locator("#table-query-run").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(4);
  await page.screenshot({ path: playwrightArtifact(`table-key-types-dedupe-${testInfo.project.name}.png`) });

  // A pair is a whole key: (a, x) is one item; b alone is every item under b.
  await keys.fill("(a, x), b");
  await expect(error).toHaveText("");
  await page.locator("#table-query-run").click();
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(2);
  await expect(page.locator("#rows-table tbody")).not.toContainText("z");
});

test("the Keys box highlights each key, alternating colors, with tuple parts apart", async ({ page, app }, testInfo) => {
  const name = uniqueTable("hl");
  app.trackTable(name);
  app.command(`CREATE TABLE ${name} (id STRING HASH KEY, sk STRING RANGE KEY);`);
  app.command(`INSERT INTO ${name} (id, sk) VALUES ('a', 'x'), ('b', 'y');`);
  await openTable(page, app, name);
  await page.locator("#table-query-keys-mode").click();
  const editor = page.locator("#table-query-batch-keys");
  await expect(editor).toHaveClass(/cm-editor/);
  const keys = page.locator('[data-testid="table-query-batch-keys"]');
  await keys.fill("(a, x), b, (c, z)\n((d, 1), (e, 2)), f, (g, h, i)");

  const first = editor.locator(".kh-hash.kh-c0").first();
  await expect(first).toHaveText("a");
  await expect(first).toHaveCSS("color", await themeColor(page, "--tok-type"));
  const second = editor.locator(".kh-hash.kh-c1").first();
  await expect(second).toHaveText("b");
  await expect(second).toHaveCSS("color", await themeColor(page, "--tok-fn"));
  await expect(editor.locator(".kh-range").first()).toHaveText("x");
  await expect(editor.locator(".kh-range").first()).toHaveCSS("color", await themeColor(page, "--tok-number"));
  await expect(editor.locator(".kh-alt-1").first()).toHaveCSS("background-color", await themeColor(page, "--hover-quiet"));
  await expect(editor.locator(".kh-punct", { hasText: "," }).first()).toHaveCSS("color", await themeColor(page, "--muted"));
  await expect(editor.locator(".kh-error").first()).toHaveCSS("color", await themeColor(page, "--danger"));
  await expect(page.locator("#table-query-error")).toContainText("at most two parts");
  await editor.screenshot({ path: playwrightArtifact(`table-keys-highlight-${testInfo.project.name}.png`) });

  // Ctrl/Cmd+Enter in the Keys box runs, as in the query editor.
  await keys.fill("(a, x), b");
  await page.keyboard.press(process.platform === "darwin" ? "Meta+Enter" : "Control+Enter");
  await expect(page.locator("#rows-table tbody tr")).toHaveCount(2);
});
