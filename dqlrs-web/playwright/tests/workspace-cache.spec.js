import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { openApp, openResultPane, pressMod, replaceEditor } from "../support/helpers.js";

// Everything dqlrs saves for a workspace is in <workspace>/.dqlrs-cache/<connection>/:
// the table cache, local table definitions, and web query results.

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

function uniqueTable() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

// Run `dqlrs -c` with `cwd` as the workspace, against the same DynamoDB Local.
function cli(app, cwd, command, extra = []) {
  const result = spawnSync(app.binary, ["-c", command, "-H", "localhost", "-p", "8000", ...extra], {
    cwd,
    encoding: "utf8",
    timeout: 30_000,
    env: {
      ...process.env,
      AWS_ACCESS_KEY_ID: "fakeid",
      AWS_SECRET_ACCESS_KEY: "fakekey",
      AWS_REGION: "us-west-1",
      AWS_EC2_METADATA_DISABLED: "true",
    },
  });
  return `${result.stdout}${result.stderr}`;
}

test("keeps local tables, the table cache, and results per workspace and connection", async ({ page, app }, testInfo) => {
  const source = uniqueTable();
  const local = uniqueTable();
  app.trackTable(source);
  fs.writeFileSync(path.join(app.workspace, "queries", "people.csv"), "id,name\na,ada\n");

  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${source};`,
      `CREATE TABLE ${source} (id STRING HASH KEY);`,
      `DROP LOCALTABLE IF EXISTS ${local};`,
      `CREATE LOCALTABLE ${local} AS './people.csv';`,
      `SCAN * FROM ${local};`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="run-status-ok"]')).toHaveCount(5);
  await openResultPane(page, "history");
  await expect(page.locator(".query-result")).toHaveCount(5);

  const cacheRoot = path.join(app.workspace, ".dqlrs-cache");
  expect(fs.readFileSync(path.join(cacheRoot, ".gitignore"), "utf8").split("\n")).toContain("*");
  expect(fs.statSync(cacheRoot).mode & 0o777).toBe(0o700);
  expect(fs.existsSync(path.join(app.workspace, ".dqlrs-web-cache"))).toBe(false);
  const connections = fs.readdirSync(cacheRoot).filter((name) => name !== ".gitignore");
  expect(connections).toHaveLength(1);
  const connection = path.join(cacheRoot, connections[0]);
  expect(connections[0]).toMatch(/^[0-9a-f]{64}$/);
  expect(fs.existsSync(path.join(connection, "local-tables.seal"))).toBe(true);
  expect(fs.existsSync(path.join(connection, "tables", "list.json"))).toBe(true);
  await expect.poll(() => fs.readdirSync(path.join(connection, "results")).length).toBe(1);

  // The REPL in the same workspace and connection sees the local table; another workspace
  // and another connection (region) in this workspace do not.
  const elsewhere = fs.mkdtempSync(path.join(os.tmpdir(), "dqlrs-other-workspace-"));
  try {
    expect(cli(app, app.workspace, "LS")).toContain(local);
    expect(cli(app, elsewhere, "LS")).not.toContain(local);
    expect(cli(app, app.workspace, "LS", ["-r", "us-east-1"])).not.toContain(local);
  } finally {
    fs.rmSync(elsewhere, { recursive: true, force: true });
  }
  expect(fs.readdirSync(cacheRoot).filter((name) => name !== ".gitignore")).toHaveLength(2);
  await page.screenshot({ path: playwrightArtifact(`workspace-cache-${testInfo.project.name}.png`) });

  await replaceEditor(page, `DROP LOCALTABLE ${local};`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="run-status-ok"]')).toHaveCount(1);
});

test("settings menu clears local tables and the result cache after a confirm", async ({ page, app }, testInfo) => {
  const local = uniqueTable();
  fs.writeFileSync(path.join(app.workspace, "queries", "people.csv"), "id,name\na,ada\n");
  await replaceEditor(page, `DROP LOCALTABLE IF EXISTS ${local};\nCREATE LOCALTABLE ${local} AS './people.csv';\nSCAN * FROM ${local};`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator('[data-testid="run-status-ok"]')).toHaveCount(3);
  await openResultPane(page, "history");
  await expect(page.locator(".query-result")).toHaveCount(3);
  const connection = path.join(
    app.workspace,
    ".dqlrs-cache",
    fs.readdirSync(path.join(app.workspace, ".dqlrs-cache")).find((name) => name !== ".gitignore"),
  );
  await expect.poll(() => fs.readdirSync(path.join(connection, "results")).length).toBe(1);

  const settings = page.locator("#settings-pop");
  await page.locator("#settings-toggle").click();
  await expect(settings).toBeVisible();
  await expect(page.locator("#clear-local-tables")).toHaveText("Clear local tables");
  await expect(page.locator("#clear-result-cache")).toHaveText("Clear result cache");

  // Cancel keeps everything and brings the settings menu back.
  await page.locator("#clear-local-tables").click();
  await expect(page.locator("#confirm-pop")).toBeVisible();
  await expect(page.locator("#confirm-pop-title")).toHaveText("Clear local tables");
  await page.locator("#confirm-cancel").click();
  await expect(settings).toBeVisible();
  await expect(page.locator("#local-tables-clear-note")).toBeHidden();
  expect(cli(app, app.workspace, "LS")).toContain(local);

  await page.locator("#clear-local-tables").click();
  await page.locator("#confirm-ok").click();
  await expect(settings).toBeVisible();
  await expect(page.locator("#local-tables-clear-note")).toHaveText("Cleared one local table.");
  expect(cli(app, app.workspace, "LS")).not.toContain(local);
  expect(fs.existsSync(path.join(app.workspace, "queries", "people.csv"))).toBe(true);

  await page.locator("#clear-result-cache").click();
  await expect(page.locator("#confirm-pop-title")).toHaveText("Clear result cache");
  await page.locator("#confirm-ok").click();
  await expect(settings).toBeVisible();
  await expect(page.locator("#result-cache-clear-note")).toHaveText("Cleared results for one file.");
  await expect(page.locator("#local-tables-clear-note")).toBeHidden();
  await expect(page.locator(".query-result")).toHaveCount(0);
  expect(fs.existsSync(path.join(connection, "results"))).toBe(false);
  await page.screenshot({ path: playwrightArtifact(`settings-clear-caches-${testInfo.project.name}.png`) });

  // The results stay cleared after a reload.
  await page.reload();
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  await openResultPane(page, "history");
  await expect(page.locator(".query-result")).toHaveCount(0);
});

