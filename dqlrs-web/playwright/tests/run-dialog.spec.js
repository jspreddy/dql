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
  await expect(page.locator("#run-pop-start .btn-label")).toHaveText("Run 2 files");
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
  await expect(page.locator("#run-pop-cancel .btn-label")).toHaveText("Close");
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
  // The INSERT writes two rows. Its units are write units, not read units.
  expect(Number(await total(page, "write-units").textContent())).toBeGreaterThan(0);
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
  await expect(page.locator("#run-pop-cancel .btn-label")).toHaveText("Close");
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
  await expect(page.locator("#run-pop-start .btn-label")).toHaveText("Run 1 file");
  await expect(page.locator('[data-testid="run-file"]')).toHaveCount(1);
  await page.locator("#run-pop-cancel").click();
  await expect(page.locator("#run-pop")).toBeHidden();
  await expect(page.locator(".node-running")).toHaveCount(0);
});

test("puts the actions under the fixed title, with shortcuts, for Run folder and Run file", async ({ page, app }, testInfo) => {
  const folder = `layout-${uniqueTable()}`;
  const files = {};
  // Enough files that the tree scrolls, so the title has to stay put above it.
  for (let index = 0; index < 40; index += 1) {
    files[`${folder}/${String(index).padStart(2, "0")}-file.dql`] = "-- nothing to run\n";
  }
  files[`${folder}/zz-last.dql`] = "SELEKT * FROM nowhere;\n";
  writeFiles(app.workspace, files);
  await expect(node(page, folder)).toBeVisible();
  await node(page, folder).click({ button: "right" });
  await page.locator("#menu-run-folder").click();

  const dialog = page.locator("#run-pop");
  await expect(dialog).toBeVisible();
  // Order: title, then actions, then the summary, then the tree.
  const top = async (selector) => (await page.locator(selector).boundingBox()).y;
  const title = await top("#run-pop .schema-pop-head");
  const actions = await top(".run-pop-actions");
  const summary = await top("#run-pop-summary");
  const tree = await top("#run-pop-body");
  expect(title).toBeLessThan(actions);
  expect(actions).toBeLessThan(summary);
  expect(summary).toBeLessThan(tree);
  // The buttons sit at the left of the action bar, Cancel then Run, and each shows its shortcut.
  const bar = await page.locator(".run-pop-actions").boundingBox();
  const cancel = await page.locator("#run-pop-cancel").boundingBox();
  expect(cancel.x - bar.x).toBeLessThan(32);
  expect((await page.locator("#run-pop-start").boundingBox()).x).toBeGreaterThan(cancel.x);
  await expect(page.locator("#run-pop-start .btn-kbd")).toHaveText(/^(Ctrl|⌘) \+ Enter$/);
  await expect(page.locator("#run-pop-cancel .btn-kbd")).toHaveText("Esc");
  await expect(page.locator("#run-pop-stop .btn-kbd")).toHaveText("Esc");
  await page.screenshot({ path: playwrightArtifact(`run-dialog-actions-${testInfo.project.name}.png`) });

  // The title stays fixed; the actions, summary, and tree scroll away under it.
  const scroll = page.locator("#run-pop-scroll");
  await scroll.evaluate((element) => { element.scrollTop = element.scrollHeight; });
  await expect.poll(() => scroll.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  expect(await top("#run-pop .schema-pop-head")).toBe(title);
  expect(await top(".run-pop-actions")).toBeLessThan(actions);
  await expect(page.locator("#run-pop-title")).toBeInViewport();
  await page.screenshot({ path: playwrightArtifact(`run-dialog-scrolled-${testInfo.project.name}.png`) });

  // Ctrl+Enter (⌘+Enter on a Mac) runs; Esc closes once the run is done.
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(page.locator("#run-pop-cancel .btn-label")).toHaveText("Close");
  await expect(row(page, `${folder}/zz-last.dql`)).toHaveAttribute("data-status", "error");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  // Run file uses the same layout, and Esc cancels without running.
  await reveal(page, `${folder}/zz-last.dql`);
  // A tree scroll closes the menu, so scroll the long tree to the file before the right-click.
  await node(page, `${folder}/zz-last.dql`).scrollIntoViewIfNeeded();
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
  await node(page, `${folder}/zz-last.dql`).click({ button: "right" });
  await page.locator("#menu-run-file").click();
  await expect(page.locator("#run-pop-kicker")).toHaveText("Run file");
  await expect(page.locator("#run-pop-start .btn-label")).toHaveText("Run 1 file");
  expect(await top(".run-pop-actions")).toBeLessThan(await top("#run-pop-summary"));
  await page.screenshot({ path: playwrightArtifact(`run-dialog-run-file-${testInfo.project.name}.png`) });
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(page.locator(".node-running")).toHaveCount(0);
});
