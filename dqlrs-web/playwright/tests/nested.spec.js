import { test, expect } from "../support/app.js";
import { openApp, pressMod, replaceEditor } from "../support/helpers.js";

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

function uniqueTable() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

async function runEditor(page, text) {
  await replaceEditor(page, text);
  await expect(page.locator("#editor-content")).toContainText(text.split("\n")[0].slice(0, 40));
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("#run")).toBeEnabled();
}

async function exampleLines(page, topic) {
  await runEditor(page, `HELP ${topic}`);
  const card = page.locator("[data-testid='help-card']");
  await expect(card.locator(".help-title")).toHaveText(`HELP ${topic.toLowerCase()}`);
  await expect(card).toContainText("WHERE can compare a.b.c and a[0].b on SELECT and SCAN.");
  const text = await card.locator("[data-testid='help-code']").nth(1).evaluate((node) => node.textContent);
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.includes("author.city"));
}

function lastStatement(line) {
  const parts = line.split(";").map((part) => part.trim()).filter(Boolean);
  return parts[parts.length - 1] || "";
}

function expectShape(line) {
  const query = lastStatement(line).toUpperCase();
  if (line.includes("tags[].name")) {
    return {
      headers: ["city", "tags[0].name", "names"],
      cells: ["London", "rust", '["rust","db"]'],
    };
  }
  if (query.startsWith("SCAN")) {
    return {
      headers: ["id", "city"],
      cells: ["a", "London"],
      missing: ["Grace", "New York"],
    };
  }
  return {
    headers: ["id", "city"],
    cells: ["a", "London"],
    missing: ["Grace"],
  };
}

test("HELP SELECT and HELP SCAN list the nested attribute examples", async ({ page }) => {
  const select = await exampleLines(page, "SELECT");
  const scan = await exampleLines(page, "SCAN");
  expect(select.map((line) => line.replaceAll(/\s+/g, " "))).toEqual(scan.map((line) => line.replaceAll(/\s+/g, " ")));
  expect(select.some((line) => line.includes("tags[].name AS names"))).toBe(true);
  expect(select.some((line) => line.includes("tags[0].name = 'rust' AND author.city.name = 'London'") && line.includes("SELECT"))).toBe(true);
  expect(select.some((line) => line.startsWith("CREATE") && line.includes("SCAN id, author.city.name AS city"))).toBe(true);
});

test("runs the nested SELECT and SCAN help examples", async ({ page, app }) => {
  const lines = await exampleLines(page, "SELECT");
  expect(lines).toHaveLength(3);

  for (const line of lines) {
    const table = uniqueTable();
    app.trackTable(table);
    const script = line.replaceAll("foobars", table);
    const shape = expectShape(script);
    await runEditor(page, script);
    const errors = page.locator('[data-testid="result-note"].error');
    if ((await errors.count()) > 0) {
      throw new Error(`unexpected error for ${script}: ${await errors.first().innerText()}`);
    }
    await expect(page.locator(".result-card").last().locator(".result-card-meta"), script).toHaveText("1 row");
    await expect(page.locator("#result-table thead th:not(.row-peek)"), script).toHaveText(shape.headers);
    await expect(page.locator("#result-table tbody td:not(.row-peek)"), script).toHaveText(shape.cells);
    for (const value of shape.missing || []) {
      await expect(page.locator("#result-table"), script).not.toContainText(value);
    }
  }
});
