import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { openApp, openResultPane, pressMod, replaceEditor } from "../support/helpers.js";

// Relative SAVE, LOAD, and local-table paths resolve against --dir (the workspace).
// A path that starts with ./ resolves against the folder of the .dql file being run.
// The process directory is a separate folder, so a file landing there is a bug.
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

test("saves JSON and CSV in the workspace and loads both back", async ({ page, app }) => {
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

  await expect(page.locator("#result-meta")).toBeEmpty();
  await expect(page.locator(".result-card").last().locator(".result-card-meta")).toHaveText("2 rows");
  await expect(page.locator("#result-table")).toContainText("alpha");
  await expect(page.locator("#result-table")).toContainText("beta");
  await expect(page.locator("#result-table")).toContainText("3");
  await expect(page.locator("#result-table")).toContainText("7");
  await openResultPane(page, "history");
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "2 rows saved" })).toHaveCount(2);
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "2 rows loaded" })).toHaveCount(1);

  const jsonPath = path.join(app.workspace, "saved.json");
  const csvPath = path.join(app.workspace, "saved.csv");
  expect(fs.existsSync(path.join(app.cwd, "saved.json"))).toBe(false);
  expect(fs.existsSync(path.join(app.cwd, "saved.csv"))).toBe(false);

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
  await expect(page.locator("#result-meta")).toBeEmpty();
  await expect(page.locator(".result-card").last().locator(".result-card-meta")).toHaveText("2 rows");
  await expect(page.locator("#result-table")).toContainText("alpha");
  await expect(page.locator("#result-table")).toContainText("beta");
  await openResultPane(page, "history");
  await expect(page.locator(".query-result").last().locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator(".query-result").nth(-2).locator('[data-testid="result-note"]').filter({ hasText: "2 rows loaded" })).toHaveCount(1);
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

    await expect(page.locator("#result-meta")).toBeEmpty();
  await expect(page.locator(".result-card").last().locator(".result-card-meta")).toHaveText("2 rows");
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

  await expect(page.locator("#result-meta")).toBeEmpty();
  await expect(page.locator(".result-card").last().locator(".result-card-meta")).toHaveText("2 rows");
  await expect(page.locator("#result-table")).toContainText("alpha");
  await expect(page.locator("#result-table")).toContainText("beta");
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  expect(fs.existsSync(path.join(app.cwd, folder))).toBe(false);

  const items = fs
    .readFileSync(path.join(app.workspace, folder, "rows.json"), "utf8")
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

test("names one saved row and one loaded row, and a failed load stays an error", async ({ page, app }) => {
  const source = uniqueTable();
  const loaded = uniqueTable();
  app.trackTable(source);
  app.trackTable(loaded);

  await runEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${source};`,
      `CREATE TABLE ${source} (id STRING HASH KEY);`,
      `INSERT INTO ${source} (id) VALUES ('a');`,
      `SCAN * FROM ${source} SAVE 'one.json';`,
      `CREATE TABLE ${loaded} (id STRING HASH KEY);`,
      `LOAD 'one.json' INTO ${loaded};`,
      `LOAD 'missing-one.json' INTO ${loaded};`,
    ].join("\n"),
  );

  await openResultPane(page, "history");
  const notes = page.locator('[data-testid="result-note"]');
  const saved = notes.filter({ hasText: "one row saved" });
  const loadedNote = notes.filter({ hasText: "one row loaded" });
  const failed = notes.filter({ hasText: `failed to open '${path.join(app.workspace, "missing-one.json")}'` });
  await expect(saved).toHaveCount(1);
  await expect(saved).toHaveClass(/ok/);
  await expect(loadedNote).toHaveCount(1);
  await expect(loadedNote).toHaveClass(/ok/);
  await expect(failed).toHaveCount(1);
  await expect(failed).toHaveClass(/error/);
});

test("SAVE reports a missing parent directory", async ({ page, app }) => {
  const source = uniqueTable();
  app.trackTable(source);

  await runEditor(
    page,
    [...seed(source), `SCAN * FROM ${source} SAVE 'nested/nope.json';`].join("\n"),
  );

  await expect(page.locator('[data-testid="result-note"].error')).toHaveText(
    `failed to create '${path.join(app.workspace, "nested/nope.json")}': No such file or directory (os error 2)`,
  );
  expect(fs.existsSync(path.join(app.cwd, "nested"))).toBe(false);
  expect(fs.existsSync(path.join(app.workspace, "nested"))).toBe(false);
});

test("resolves ./ paths against the .dql file's folder and other paths against the workspace", async ({ page, app }, testInfo) => {
  const source = uniqueTable();
  const beside = uniqueTable();
  const top = uniqueTable();
  const local = uniqueTable();
  app.trackTable(source);
  app.trackTable(beside);
  app.trackTable(top);
  fs.writeFileSync(path.join(app.workspace, "queries", "people.csv"), "id,name\nx,ex\ny,why\n");

  // openApp opens queries/edit.dql, so ./ means the queries folder.
  await runEditor(
    page,
    [
      ...seed(source),
      `SCAN * FROM ${source} SAVE './beside.json';`,
      `SCAN * FROM ${source} SAVE 'top.json';`,
      `CREATE TABLE ${beside} (id STRING HASH KEY);`,
      `LOAD './beside.json' INTO ${beside};`,
      `CREATE TABLE ${top} (id STRING HASH KEY);`,
      `LOAD 'top.json' INTO ${top};`,
      `DROP LOCALTABLE IF EXISTS ${local};`,
      `CREATE LOCALTABLE ${local} AS './people.csv';`,
      `SCAN * FROM ${local};`,
    ].join("\n"),
  );

  await expect(page.locator(".result-card").last().locator(".result-card-meta")).toHaveText("2 rows");
  await expect(page.locator("#result-table")).toContainText("why");
  await openResultPane(page, "history");
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "2 rows saved" })).toHaveCount(2);
  await expect(page.locator('[data-testid="result-note"]').filter({ hasText: "2 rows loaded" })).toHaveCount(2);

  expect(fs.existsSync(path.join(app.workspace, "queries", "beside.json"))).toBe(true);
  expect(fs.existsSync(path.join(app.workspace, "beside.json"))).toBe(false);
  expect(fs.existsSync(path.join(app.workspace, "top.json"))).toBe(true);
  expect(fs.existsSync(path.join(app.workspace, "queries", "top.json"))).toBe(false);
  for (const name of ["beside.json", "top.json"]) {
    expect(fs.existsSync(path.join(app.cwd, name))).toBe(false);
  }
  await page.screenshot({ path: playwrightArtifact(`relative-paths-${testInfo.project.name}.png`) });
  await runEditor(page, `DROP LOCALTABLE ${local};`);
});
