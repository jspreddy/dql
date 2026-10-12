import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test, expect } from "../support/app.js";
import { openApp, pressMod, replaceEditor } from "../support/helpers.js";

// Relative SAVE and LOAD paths follow the dqlrs process directory, not --dir.
test.use({ isolateCwd: true });

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

function seed(source) {
  return [
    `DROP TABLE IF EXISTS ${source};`,
    `CREATE TABLE ${source} (id STRING HASH KEY);`,
    `INSERT INTO ${source} (id, name, qty) VALUES ('a', 'alpha', 3);`,
    `INSERT INTO ${source} (id, name, qty) VALUES ('b', 'beta', 7);`,
  ];
}

test("saves JSON and CSV in the process directory and loads both back", async ({ page, app }) => {
  const source = uniqueTable();
  const fromJson = uniqueTable();
  const fromCsv = uniqueTable();
  app.trackTable(source);
  app.trackTable(fromJson);
  app.trackTable(fromCsv);

  await runEditor(
    page,
    [
      ...seed(source),
      `SCAN * FROM ${source} SAVE 'saved.json';`,
      `SCAN * FROM ${source} SAVE 'saved.csv';`,
      `CREATE TABLE ${fromJson} (id STRING HASH KEY);`,
      `LOAD 'saved.json' INTO ${fromJson};`,
      `SCAN * FROM ${fromJson};`,
    ].join("\n"),
  );

  await expect(page.locator("#result-meta")).toHaveText("2 rows");
  await expect(page.locator("#result-table")).toContainText("alpha");
  await expect(page.locator("#result-table")).toContainText("beta");
  await expect(page.locator("#result-table")).toContainText("3");
  await expect(page.locator("#result-table")).toContainText("7");
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "2 affected" })).toHaveCount(3);

  const jsonPath = path.join(app.cwd, "saved.json");
  const csvPath = path.join(app.cwd, "saved.csv");
  expect(fs.existsSync(path.join(app.workspace, "saved.json"))).toBe(false);
  expect(fs.existsSync(path.join(app.workspace, "saved.csv"))).toBe(false);

  const items = fs.readFileSync(jsonPath, "utf8").trim().split("\n").map((line) => JSON.parse(line));
  expect(items).toHaveLength(2);
  expect(items).toEqual(
    expect.arrayContaining([
      { id: "a", name: "alpha", qty: 3 },
      { id: "b", name: "beta", qty: 7 },
    ]),
  );

  const csv = fs.readFileSync(csvPath, "utf8").trim().split(/\r?\n/);
  expect(csv[0]).toBe("id,name,qty");
  expect(csv.slice(1).sort()).toEqual(["a,alpha,3", "b,beta,7"]);

  await runEditor(
    page,
    [
      `CREATE TABLE ${fromCsv} (id STRING HASH KEY);`,
      `LOAD 'saved.csv' INTO ${fromCsv};`,
      `SCAN * FROM ${fromCsv};`,
    ].join("\n"),
  );
  await expect(page.locator("#result-meta")).toHaveText("2 rows");
  await expect(page.locator("#result-table")).toContainText("alpha");
  await expect(page.locator("#result-table")).toContainText("beta");
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "2 affected" })).toHaveCount(1);
});

test("saves and loads an absolute path", async ({ page, app }) => {
  const source = uniqueTable();
  const loaded = uniqueTable();
  app.trackTable(source);
  app.trackTable(loaded);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "dqlrs-save-abs-"));
  const file = path.join(dir, "rows.json");
  try {
    await runEditor(
      page,
      [
        ...seed(source),
        `SCAN * FROM ${source} SAVE '${file}';`,
        `CREATE TABLE ${loaded} (id STRING HASH KEY);`,
        `LOAD '${file}' INTO ${loaded};`,
        `SCAN * FROM ${loaded};`,
      ].join("\n"),
    );

    await expect(page.locator("#result-meta")).toHaveText("2 rows");
    await expect(page.locator("#result-table")).toContainText("alpha");
    await expect(page.locator("#result-table")).toContainText("beta");
    await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
    expect(fs.existsSync(path.join(app.cwd, "rows.json"))).toBe(false);
    expect(fs.existsSync(path.join(app.workspace, "rows.json"))).toBe(false);

    const items = fs.readFileSync(file, "utf8").trim().split("\n").map((line) => JSON.parse(line));
    expect(items).toEqual(
      expect.arrayContaining([
        { id: "a", name: "alpha", qty: 3 },
        { id: "b", name: "beta", qty: 7 },
      ]),
    );
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("SAVE CREATE PARENTS writes into a new directory", async ({ page, app }) => {
  const source = uniqueTable();
  const loaded = uniqueTable();
  app.trackTable(source);
  app.trackTable(loaded);
  const folder = `exports-${source}`;

  await runEditor(
    page,
    [
      ...seed(source),
      `SCAN * FROM ${source} SAVE '${folder}/rows.json' CREATE PARENTS;`,
      `CREATE TABLE ${loaded} (id STRING HASH KEY);`,
      `LOAD '${folder}/rows.json' INTO ${loaded};`,
      `SCAN * FROM ${loaded};`,
    ].join("\n"),
  );

  await expect(page.locator("#result-meta")).toHaveText("2 rows");
  await expect(page.locator("#result-table")).toContainText("alpha");
  await expect(page.locator("#result-table")).toContainText("beta");
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  expect(fs.existsSync(path.join(app.workspace, folder))).toBe(false);

  const items = fs
    .readFileSync(path.join(app.cwd, folder, "rows.json"), "utf8")
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  expect(items).toEqual(
    expect.arrayContaining([
      { id: "a", name: "alpha", qty: 3 },
      { id: "b", name: "beta", qty: 7 },
    ]),
  );
});

test("SAVE reports a missing parent directory", async ({ page, app }) => {
  const source = uniqueTable();
  app.trackTable(source);

  await runEditor(
    page,
    [...seed(source), `SCAN * FROM ${source} SAVE 'nested/nope.json';`].join("\n"),
  );

  await expect(page.locator('[data-testid="result-note"].error')).toHaveText(
    "failed to create 'nested/nope.json': No such file or directory (os error 2)",
  );
  expect(fs.existsSync(path.join(app.cwd, "nested"))).toBe(false);
  expect(fs.existsSync(path.join(app.workspace, "nested"))).toBe(false);
});
