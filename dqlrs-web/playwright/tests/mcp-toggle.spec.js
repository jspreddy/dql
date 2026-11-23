import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { openApp } from "../support/helpers.js";

// The MCP pill next to the endpoint shows whether /mcp answers and turns it on and off. The
// server starts without --mcp here, so the pill starts off.

const INITIALIZE = {
  jsonrpc: "2.0",
  id: 1,
  method: "initialize",
  params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "pw", version: "0" } },
};

async function mcpStatus(page, app) {
  const token = await page.locator('meta[name="dqlrs-token"]').getAttribute("content");
  const response = await page.request.post(`${app.baseURL}/mcp`, {
    headers: { Authorization: `Bearer ${token}`, Accept: "application/json, text/event-stream" },
    data: INITIALIZE,
  });
  return response.status();
}

test("the MCP pill shows whether MCP is on and turns it on and off in every tab", async ({ page, app, context }, testInfo) => {
  await openApp(page, app);
  const pill = page.locator("#mcp-toggle");
  await expect(pill).toBeVisible();
  await expect(pill).toHaveText("MCP off");
  await expect(pill).toHaveAttribute("aria-pressed", "false");
  await expect(pill).toHaveAttribute("data-tip", /Agents cannot connect\. Click to turn MCP on\./);
  // It sits just before the endpoint pill.
  await expect(page.locator(".header-tools > #mcp-toggle + #endpoint")).toHaveCount(1);
  expect(await mcpStatus(page, app)).toBe(404);
  await page.locator(".header-tools").screenshot({
    path: playwrightArtifact(`mcp-toggle-off-${testInfo.project.name}.png`),
  });

  const other = await context.newPage();
  await openApp(other, app);
  const otherPill = other.locator("#mcp-toggle");
  await expect(otherPill).toHaveText("MCP off");

  await pill.click();
  await expect(pill).toHaveText("MCP on");
  await expect(pill).toHaveAttribute("aria-pressed", "true");
  await expect(pill).toHaveAttribute("data-tip", new RegExp(`Agents can connect at ${app.baseURL}/mcp\\.`));
  expect(await mcpStatus(page, app)).toBe(200);
  // The other tab hears the change without a reload.
  await expect(otherPill).toHaveText("MCP on");
  await expect(otherPill).toHaveAttribute("aria-pressed", "true");
  await page.locator(".header-tools").screenshot({
    path: playwrightArtifact(`mcp-toggle-on-${testInfo.project.name}.png`),
  });

  await otherPill.click();
  await expect(otherPill).toHaveText("MCP off");
  await expect(pill).toHaveText("MCP off");
  expect(await mcpStatus(page, app)).toBe(404);
  // A reload reads the state from the server.
  await page.reload();
  await expect(page.locator("#mcp-toggle")).toHaveText("MCP off");
  await other.close();
});

test.describe("started with --mcp", () => {
  test.use({ mcp: true });

  test("the MCP pill starts on", async ({ page, app }) => {
    await openApp(page, app);
    await expect(page.locator("#mcp-toggle")).toHaveText("MCP on");
    expect(await mcpStatus(page, app)).toBe(200);
  });
});
