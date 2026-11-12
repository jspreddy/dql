import { test, expect } from "../../support/app.js";
import { playwrightArtifact } from "../../support/artifacts.js";
import { openComponentHarness } from "../../support/components.js";

// run-tree.js: the run dialog's file tree, dots, summaries, and totals.

test.beforeEach(async ({ page, app }, testInfo) => {
  await openComponentHarness(page, app, testInfo);
});

// Mount a tree for `paths` under folder "batch" and keep the files on window.files.
async function mountTree(page, paths) {
  await page.evaluate(async (list) => {
    const { buildRunTree } = await import("/run-tree.js");
    const { emptyCounts } = await import("/statement-text.js");
    window.files = list.map(([path, statements]) => ({
      doc: { path },
      statements,
      counts: emptyCounts(),
      status: statements ? "waiting" : "skipped",
      error: "",
      row: null,
    }));
    const totals = document.createElement("div");
    totals.className = "run-pop-totals";
    totals.id = "totals";
    totals.hidden = true;
    document.querySelector("#root").replaceChildren(totals, buildRunTree("batch", "dir", window.files));
  }, paths);
}

const row = (page, path) => page.locator(`[data-testid="run-file"][data-path="${path}"]`);

test("lists a folder's files before its subfolders, in run order", async ({ page }) => {
  await mountTree(page, [
    ["batch/a.dql", 2],
    ["batch/b.dql", 1],
    ["batch/sub/c.dql", 3],
    ["batch/sub/deeper/d.dql", 0],
  ]);
  await expect(page.locator('[data-testid="run-file"] .run-tree-name')).toHaveText(["a.dql", "b.dql", "c.dql", "d.dql"]);
  await expect(page.locator(".run-tree-dir > .run-tree-row")).toHaveText(["sub/", "deeper/"]);
  await expect(row(page, "batch/a.dql").locator(".run-tree-count")).toHaveText("2 statements");
  await expect(row(page, "batch/b.dql").locator(".run-tree-count")).toHaveText("one statement");
  await expect(row(page, "batch/sub/deeper/d.dql")).toHaveAttribute("data-status", "skipped");
  await expect(row(page, "batch/sub/deeper/d.dql").locator(".run-tree-summary")).toHaveText("Skipped");
  await expect(row(page, "batch/a.dql").locator(".run-tree-summary")).toHaveText("");
});

test("paints each status and renders the totals", async ({ page }, testInfo) => {
  await mountTree(page, [
    ["batch/ok.dql", 2],
    ["batch/bad.dql", 1],
    ["batch/running.dql", 2],
    ["batch/later.dql", 1],
  ]);
  await page.evaluate(async () => {
    const { paintRunFile, renderRunTotals } = await import("/run-tree.js");
    const { addResultCounts, emptyCounts } = await import("/statement-text.js");
    const [ok, bad, running, later] = window.files;
    const total = emptyCounts();
    for (const result of [
      { statement: "SCAN * FROM t", ok: true, read_units: 1.25 },
      { statement: "INSERT INTO t (id) VALUES ('a')", ok: true, write_units: 1 },
    ]) {
      addResultCounts(ok.counts, result);
      addResultCounts(total, result);
    }
    ok.status = "ok";
    const failure = { statement: "SELEKT nope", ok: false, error: "expected a DQL statement" };
    addResultCounts(bad.counts, failure);
    addResultCounts(total, failure);
    bad.status = "error";
    bad.error = failure.error;
    running.status = "running";
    later.status = "skipped";
    for (const file of window.files) paintRunFile(file);
    renderRunTotals(document.querySelector("#totals"), total);
  });
  await expect(row(page, "batch/ok.dql").locator(".run-dot")).toHaveAttribute("data-status", "ok");
  await expect(row(page, "batch/ok.dql").locator(".run-tree-summary")).toHaveText("2 queries · one read · one write · 2 ok");
  await expect(row(page, "batch/bad.dql").locator(".run-tree-summary")).toHaveText("one query · 0 reads · 0 writes · 0 ok · 1 failed");
  await expect(row(page, "batch/bad.dql").locator(".run-tree-summary")).toHaveClass(/is-error/);
  await expect(row(page, "batch/bad.dql").locator(".run-tree-summary")).toHaveAttribute("data-tip", "expected a DQL statement");
  await expect(row(page, "batch/running.dql").locator(".run-tree-summary")).toHaveText("Running…");
  await expect(row(page, "batch/later.dql").locator(".run-tree-summary")).toHaveText("Not run");
  // The dot colors differ by status.
  const colors = await page.locator(".run-dot").evaluateAll((dots) => dots.map((dot) => getComputedStyle(dot).backgroundColor));
  expect(new Set(colors.slice(0, 3)).size).toBe(3);

  const totals = page.locator("#totals");
  await expect(totals).toBeVisible();
  await expect(totals.locator(".run-total-label")).toHaveText(["Queries", "Reads", "Writes", "Success", "Failure", "Read units", "Write units"]);
  await expect(totals.locator(".run-total-value")).toHaveText(["3", "1", "1", "2", "1", "1.25", "1"]);
  await expect(page.locator('[data-testid="run-total-failure"]')).not.toHaveAttribute("data-zero", "");
  await page.screenshot({ path: playwrightArtifact(`component-run-tree-${testInfo.project.name}.png`) });
});

test("shows an empty note for a folder with no files", async ({ page }) => {
  await page.evaluate(async () => {
    const { buildRunTree } = await import("/run-tree.js");
    document.querySelector("#root").replaceChildren(buildRunTree("empty", "dir", []));
  });
  await expect(page.locator("#root .empty")).toHaveText("This folder has no .dql files.");
});
