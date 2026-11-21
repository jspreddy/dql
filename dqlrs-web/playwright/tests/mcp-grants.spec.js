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

test("a request pings the mcp icon, and a click opens the grant panel", async ({ page, app }, testInfo) => {
  await openApp(page, app);
  const source = uniqueName();
  app.trackTable(source);
  await replaceEditor(
    page,
    [
      `DROP TABLE IF EXISTS ${source};`,
      `CREATE TABLE ${source} (id STRING HASH KEY, email STRING);`,
      `INSERT INTO ${source} (id, email) VALUES ('1', 'ada@example.com');`,
    ].join("\n"),
  );
  await pressMod(page, "A");
  await page.locator("#run").click();
  await expect(page.locator("[data-testid='result-note']", { hasText: "one row inserted" })).toBeVisible();

  const token = await page.locator('meta[name="dqlrs-token"]').getAttribute("content");
  const headers = {
    Authorization: `Bearer ${token}`,
    "Content-Type": "application/json",
  };
  const init = await page.request.post(`${app.baseURL}/mcp`, {
    headers,
    data: {
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: {
        protocolVersion: "2025-06-18",
        capabilities: {},
        clientInfo: { name: "playwright", version: "0" },
      },
    },
  });
  expect(init.ok()).toBeTruthy();

  const query = `SCAN * FROM ${source}`;
  const ask = () => page.evaluate(async ({ token, query }) => {
    const response = await fetch("/mcp", {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 2,
        method: "tools/call",
        params: { name: "request_access", arguments: { query } },
      }),
    });
    return response.json();
  }, { token, query });

  const panel = page.locator("#grant-panel");
  const deniedPending = ask();
  await expect(panel).toBeHidden();
  const icon = page.locator(".presence-icon.is-ping");
  await expect(icon).toHaveCount(1);
  await expect(icon).toHaveCSS("animation-name", "presence-icon-ping");
  await icon.click();
  await expect(panel).toBeVisible();
  await expect(panel).toContainText(query);
  await page.locator("#grant-panel-close").click();
  await expect(panel).toBeHidden();
  await expect(icon).toHaveClass(/is-ping/);
  await icon.click();
  await expect(panel).toBeVisible();
  await panel.getByRole("button", { name: "Deny" }).click();
  const deniedBody = await deniedPending;
  expect(deniedBody.result.isError).toBe(false);
  expect(JSON.parse(deniedBody.result.content[0].text).status).toBe("denied");
  await expect(panel).toContainText("No pending requests.");

  const grantedPending = ask();
  await expect(panel.getByRole("button", { name: "Run" })).toBeVisible();
  await panel.getByRole("button", { name: "Run" }).click();
  await expect(panel).toContainText("Recommend not granting");
  await expect(panel).toContainText("ada@example.com");
  await expect(panel).toContainText("email");
  await expect(panel.getByRole("button", { name: "Run again", exact: true })).toBeVisible();
  await expect(panel.getByText("Granted", { exact: true })).toBeVisible();
  await panel.screenshot({
    path: playwrightArtifact(`mcp-access-panel-${testInfo.project.name}.png`),
  });
  await panel.getByRole("button", { name: "Grant" }).click();
  const grantedBody = await grantedPending;
  expect(grantedBody.result.isError).toBe(false);
  const granted = JSON.parse(grantedBody.result.content[0].text);
  expect(granted.status).toBe("granted");
  expect(granted.table).toMatch(/^mcp_r/);

  const read = await page.request.post(`${app.baseURL}/mcp`, {
    headers,
    data: {
      jsonrpc: "2.0",
      id: 3,
      method: "tools/call",
      params: {
        name: "read_granted_table",
        arguments: { table: granted.table, limit: 10 },
      },
    },
  });
  const readBody = await read.json();
  expect(readBody.result.isError).toBe(false);
  const rows = JSON.parse(readBody.result.content[0].text);
  expect(rows.total).toBe(1);
  expect(rows.rows[0].email).toBe("ada@example.com");
});
