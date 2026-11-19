import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { openApp, openResultPane, pressMod, replaceEditor } from "../support/helpers.js";

test.use({ mcp: true });

function uniqueName() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

test("grants a temp table, reads it over MCP, then revokes it", async ({ page, app }, testInfo) => {
  await openApp(page, app);
  const source = uniqueName();
  const temp = `late${source}`;
  app.trackTable(source);

  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${source};`,
      `CREATE TABLE ${source} (id STRING HASH KEY, status STRING);`,
      `INSERT INTO ${source} (id, status) VALUES ('1', 'late'), ('2', 'ok'), ('3', 'late');`,
      `CREATE TEMPTABLE ${temp} AS (SCAN * FROM ${source} WHERE status = 'late');`,
      `GRANT MCP READ ON TEMPTABLE ${temp};`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  const granted = page.locator("[data-testid='result-note']", { hasText: "Granted MCP read" });
  await expect(granted).toBeVisible();
  await expect(granted).toContainText(`temp table ${temp} (2 rows)`);
  await expect(granted).not.toContainText("No agent can read it here");

  await page.locator("#mode-tables").click();
  await page.locator("#table-search").evaluate((input, value) => {
    input.value = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }, temp);
  const row = page.locator(`#table-${temp}`);
  const marker = row.locator("[data-kind='mcp']");
  await expect(marker).toBeVisible();
  await expect(marker).toHaveText("mcp");
  await row.screenshot({
    path: playwrightArtifact(`mcp-grant-marker-${testInfo.project.name}.png`),
  });

  const token = await page.locator('meta[name="dqlrs-token"]').getAttribute("content");
  const call = (body) => page.request.post(`${app.baseURL}/mcp`, {
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
    data: body,
  });
  const init = await call({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: {
      protocolVersion: "2025-06-18",
      capabilities: {},
      clientInfo: { name: "playwright", version: "0" },
    },
  });
  expect(init.ok()).toBeTruthy();

  const read = await call({
    jsonrpc: "2.0",
    id: 2,
    method: "tools/call",
    params: {
      name: "read_granted_table",
      arguments: { table: temp, limit: 10 },
    },
  });
  const body = await read.json();
  expect(body.result.isError).toBe(false);
  const payload = JSON.parse(body.result.content[0].text);
  expect(payload.total).toBe(2);
  expect(payload.rows).toHaveLength(2);
  expect(payload.rows.map((item) => item.status).sort()).toEqual(["late", "late"]);

  await page.locator("#mode-query").click();
  await openResultPane(page, "history");
  await expect(page.locator("[data-testid='result-note']", { hasText: `MCP read ${temp}: 2 rows` })).toBeVisible();
  await page.locator("#result-body").screenshot({
    path: playwrightArtifact(`mcp-grant-history-${testInfo.project.name}.png`),
  });

  await replaceEditor(page, `REVOKE MCP READ ON TEMPTABLE ${temp};`);
  await page.locator("#run").click();
  await expect(page.locator("[data-testid='result-note']", { hasText: "Revoked MCP read" })).toBeVisible();
  await page.locator("#mode-tables").click();
  await expect(row.locator("[data-kind='mcp']")).toHaveCount(0);

  const denied = await call({
    jsonrpc: "2.0",
    id: 3,
    method: "tools/call",
    params: {
      name: "read_granted_table",
      arguments: { table: temp },
    },
  });
  const deniedBody = await denied.json();
  expect(deniedBody.result.isError).toBe(true);
});
