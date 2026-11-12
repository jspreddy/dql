import fs from "node:fs";
import path from "node:path";
import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { node, openApp, reveal } from "../support/helpers.js";

// "Run folder" and "Run file" in the tree menu list the files as a tree, run them after a
// confirm, and show a dot and a summary per file plus totals for the run.

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

function uniqueTable() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

function writeFiles(workspace, files) {
  for (const [relative, text] of Object.entries(files)) {
    const file = path.join(workspace, relative);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, text);
  }
}

const row = (page, file) => page.locator(`[data-testid="run-file"][data-path="${file}"]`);
const total = (page, key) => page.locator(`[data-testid="run-total-${key}"] .run-total-value`);

test("runs a folder from a confirmed tree and totals the queries", async ({ page, app }, testInfo) => {
  const table = uniqueTable();
  app.trackTable(table);
  const folder = `batch-${table}`;
  writeFiles(app.workspace, {
    [`${folder}/a-seed.dql`]: [
      `DROP TABLE IF EXISTS ${table};`,
      `CREATE TABLE ${table} (id STRING HASH KEY);`,
      `INSERT INTO ${table} (id) VALUES ('a'), ('b');`,
      `SCAN * FROM ${table};`,
    ].join("\n"),
    [`${folder}/nested/b-read.dql`]: [
      `SELECT * FROM ${table} WHERE id = 'a';`,
      `-- SAVE writes a file, so it counts as a write too.`,
      `SCAN * FROM ${table} SAVE './out.json';`,
    ].join("\n"),
    [`${folder}/nested/c-empty.dql`]: "-- nothing to run here\n",
  });
  await expect(node(page, folder)).toBeVisible();
  await node(page, folder).click({ button: "right" });
  await page.locator("#menu-run-folder").click();

  const dialog = page.locator("#run-pop");
  await expect(dialog).toBeVisible();
  await expect(page.locator("#run-pop-kicker")).toHaveText("Run folder");
  await expect(page.locator("#run-pop-title")).toHaveText(folder);
  await expect(page.locator("#run-pop-summary")).toContainText("2 files · 6 queries");
  await expect(page.locator("#run-pop-start")).toHaveText("Run 2 files");
  // The tree shows the nested folder, each file's statement count, and nothing run yet.
  await expect(page.locator(".run-tree-dir > .run-tree-row")).toHaveText(["nested/"]);
  // The tree lists files in run order: a folder's own files, then its subfolders.
  await expect(page.locator('[data-testid="run-file"] .run-tree-name')).toHaveText([
    "a-seed.dql",
    "b-read.dql",
    "c-empty.dql",
  ]);
  await expect(row(page, `${folder}/a-seed.dql`).locator(".run-tree-count")).toHaveText("4 statements");
  await expect(row(page, `${folder}/nested/b-read.dql`).locator(".run-tree-count")).toHaveText("2 statements");
  await expect(row(page, `${folder}/nested/c-empty.dql`).locator(".run-tree-count")).toHaveText("no statements");
  await expect(row(page, `${folder}/a-seed.dql`)).toHaveAttribute("data-status", "waiting");
  await expect(row(page, `${folder}/nested/c-empty.dql`)).toHaveAttribute("data-status", "skipped");
  await expect(page.locator("#run-pop-totals")).toBeHidden();
  await page.screenshot({ path: playwrightArtifact(`run-dialog-confirm-${testInfo.project.name}.png`) });
  // Nothing ran before the confirm.
  expect(fs.existsSync(path.join(app.workspace, folder, "nested", "out.json"))).toBe(false);

  await page.locator("#run-pop-start").click();
  await expect(page.locator("#run-pop-cancel")).toHaveText("Close");
  await expect(row(page, `${folder}/a-seed.dql`)).toHaveAttribute("data-status", "ok");
  await expect(row(page, `${folder}/nested/b-read.dql`)).toHaveAttribute("data-status", "ok");
  await expect(row(page, `${folder}/nested/c-empty.dql`)).toHaveAttribute("data-status", "skipped");
  await expect(row(page, `${folder}/a-seed.dql`).locator(".run-tree-summary")).toHaveText(
    "4 queries · one read · 3 writes · 4 ok",
  );
  await expect(row(page, `${folder}/nested/b-read.dql`).locator(".run-tree-summary")).toHaveText(
    "2 queries · 2 reads · one write · 2 ok",
  );
  await expect(total(page, "queries")).toHaveText("6");
  await expect(total(page, "reads")).toHaveText("3");
  await expect(total(page, "writes")).toHaveText("4");
  await expect(total(page, "success")).toHaveText("6");
  await expect(total(page, "failure")).toHaveText("0");
  await expect(total(page, "read-units")).toHaveText(/^\d+(\.\d+)?$/);
  await expect(total(page, "write-units")).toHaveText(/^\d+(\.\d+)?$/);
  await expect(page.locator("#run-pop-status")).toHaveText("Ran 2 files.");
  expect(fs.existsSync(path.join(app.workspace, folder, "nested", "out.json"))).toBe(true);
  await page.screenshot({ path: playwrightArtifact(`run-dialog-done-${testInfo.project.name}.png`) });

  await page.locator("#run-pop-cancel").click();
  await expect(dialog).toBeHidden();
});

test("stops at a failed file, marks the rest not run, and runs a single file", async ({ page, app }) => {
  const folder = `fails-${uniqueTable()}`;
  writeFiles(app.workspace, {
    [`${folder}/1-bad.dql`]: "SELEKT * FROM nowhere;\n",
    [`${folder}/2-later.dql`]: "SCAN * FROM nowhere_either;\n",
  });
  await expect(node(page, folder)).toBeVisible();
  await node(page, folder).click({ button: "right" });
  await page.locator("#menu-run-folder").click();
  await page.locator("#run-pop-start").click();
  await expect(page.locator("#run-pop-cancel")).toHaveText("Close");
  await expect(row(page, `${folder}/1-bad.dql`)).toHaveAttribute("data-status", "error");
  await expect(row(page, `${folder}/1-bad.dql`).locator(".run-tree-summary")).toHaveClass(/is-error/);
  await expect(row(page, `${folder}/2-later.dql`)).toHaveAttribute("data-status", "skipped");
  await expect(row(page, `${folder}/2-later.dql`).locator(".run-tree-summary")).toHaveText("Not run");
  await expect(total(page, "failure")).toHaveText("1");
  await expect(total(page, "success")).toHaveText("0");
  await expect(page.locator("#run-pop-status")).toHaveText("One file failed of 1 run; 1 not run.");
  await page.locator("#run-pop-cancel").click();

  // Run file lists just that file, and Cancel runs nothing.
  await reveal(page, `${folder}/2-later.dql`);
  await node(page, `${folder}/2-later.dql`).click({ button: "right" });
  await page.locator("#menu-run-file").click();
  await expect(page.locator("#run-pop-kicker")).toHaveText("Run file");
  await expect(page.locator("#run-pop-start")).toHaveText("Run 1 file");
  await expect(page.locator('[data-testid="run-file"]')).toHaveCount(1);
  await page.locator("#run-pop-cancel").click();
  await expect(page.locator("#run-pop")).toBeHidden();
  await expect(page.locator(".node-running")).toHaveCount(0);
});
