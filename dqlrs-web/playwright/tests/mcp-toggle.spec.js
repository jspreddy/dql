import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { openApp } from "../support/helpers.js";

// The MCP pill next to the endpoint shows whether /mcp answers. It opens a menu with the on/off
// switch, what MCP does, the address, and the Cursor or Claude config to copy. The server
// starts without --mcp here, so it starts off.

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

test("the MCP pill opens a menu that turns MCP on and off in every tab and copies the config", async ({ page, app, context }, testInfo) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin: app.baseURL });
  await openApp(page, app);
  const token = await page.locator('meta[name="dqlrs-token"]').getAttribute("content");
  const pill = page.locator("#mcp-toggle");
  const menu = page.locator("#mcp-pop");
  const toggle = page.locator("#mcp-switch");
  await expect(pill).toHaveText("MCP off");
  await expect(pill).toHaveAttribute("aria-expanded", "false");
  // It sits just before the endpoint pill.
  await expect(page.locator(".header-tools > #mcp-toggle + #endpoint")).toHaveCount(1);
  expect(await mcpStatus(page, app)).toBe(404);

  const other = await context.newPage();
  await openApp(other, app);
  await expect(other.locator("#mcp-toggle")).toHaveText("MCP off");

  // The pill opens the menu. It does not switch MCP by itself.
  await pill.click();
  await expect(menu).toBeVisible();
  await expect(pill).toHaveAttribute("aria-expanded", "true");
  await expect(pill).toHaveText("MCP off");
  await expect(toggle).not.toBeChecked();
  await expect(menu.locator(".mcp-pop-about")).toContainText("cannot run a query");
  await expect(page.locator("#mcp-pop-url")).toHaveText(`${app.baseURL}/mcp`);
  // The menu opens under the pill, inside the window.
  const pillBox = await pill.boundingBox();
  const menuBox = await menu.boundingBox();
  expect(menuBox.y).toBeGreaterThan(pillBox.y + pillBox.height);
  expect(menuBox.x + menuBox.width).toBeLessThanOrEqual(page.viewportSize().width);

  // Cursor's config by default: the address and this server's token.
  await expect(page.locator("#mcp-client-cursor")).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#mcp-pop-file")).toHaveText(".cursor/mcp.json");
  const cursorText = await page.locator("#mcp-pop-config").textContent();
  const cursor = JSON.parse(cursorText);
  expect(cursor.mcpServers.dqlrs.url).toBe(`${app.baseURL}/mcp`);
  expect(cursor.mcpServers.dqlrs.headers.Authorization).toBe(`Bearer ${token}`);
  await page.locator("#mcp-copy").click();
  await expect(page.locator("#mcp-copy")).toHaveAttribute("data-copied", "yes");
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(cursorText);

  // Claude's config adds the transport type and goes in .mcp.json.
  await page.locator("#mcp-client-claude").click();
  await expect(page.locator("#mcp-client-claude")).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#mcp-pop-file")).toHaveText(".mcp.json");
  const claudeText = await page.locator("#mcp-pop-config").textContent();
  expect(JSON.parse(claudeText).mcpServers.dqlrs.type).toBe("http");
  await page.locator("#mcp-copy").click();
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(claudeText);

  // The switch turns MCP on. The pill and the other tab follow.
  await menu.locator(".mcp-switch").click();
  await expect(toggle).toBeChecked();
  await expect(pill).toHaveText("MCP on");
  expect(await mcpStatus(page, app)).toBe(200);
  await expect(other.locator("#mcp-toggle")).toHaveText("MCP on");
  await page.screenshot({
    path: playwrightArtifact(`mcp-menu-on-${testInfo.project.name}.png`),
    clip: { x: menuBox.x - 220, y: 0, width: menuBox.width + 230, height: menuBox.y + menuBox.height + 10 },
  });

  // Escape closes the menu; the switch in the other tab turns MCP off for both.
  await page.keyboard.press("Escape");
  await expect(menu).toBeHidden();
  await expect(pill).toHaveAttribute("aria-expanded", "false");
  await other.locator("#mcp-toggle").click();
  await expect(other.locator("#mcp-switch")).toBeChecked();
  await other.locator("#mcp-pop .mcp-switch").click();
  await expect(other.locator("#mcp-toggle")).toHaveText("MCP off");
  await expect(pill).toHaveText("MCP off");
  expect(await mcpStatus(page, app)).toBe(404);
  await page.reload();
  await expect(page.locator("#mcp-toggle")).toHaveText("MCP off");
  await other.close();
});

test.describe("started with --mcp", () => {
  test.use({ mcp: true });

  test("the MCP pill starts on and its switch is checked", async ({ page, app }) => {
    await openApp(page, app);
    await expect(page.locator("#mcp-toggle")).toHaveText("MCP on");
    await page.locator("#mcp-toggle").click();
    await expect(page.locator("#mcp-switch")).toBeChecked();
    expect(await mcpStatus(page, app)).toBe(200);
  });
});
