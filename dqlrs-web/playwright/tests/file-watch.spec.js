import fs from "node:fs";
import path from "node:path";
import { test, expect } from "../support/app.js";
import { openApp, replaceEditor } from "../support/helpers.js";

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

test("refreshes a clean editor when the file changes on disk", async ({ page, app }) => {
  const file = path.join(app.workspace, "queries/edit.dql");
  fs.writeFileSync(file, "-- editable\nSELECT 1;\n-- from ide\n");
  await expect(page.locator("#editor-content")).toContainText("from ide");
  await expect(page.locator("#patch-pop")).toBeHidden();
  await page.waitForTimeout(1200);
  const disk = fs.readFileSync(file, "utf8");
  expect(disk).toContain("from ide");
  expect(disk).not.toContain("local unsaved");
});

test("keeps unsaved edits as a patch when the file changes on disk", async ({ page, app }) => {
  await page.route("**/api/file", async (route) => {
    if (route.request().method() === "PUT") {
      await new Promise((resolve) => setTimeout(resolve, 1500));
    }
    await route.continue();
  });
  const file = path.join(app.workspace, "queries/edit.dql");
  await replaceEditor(page, "-- editable\nSELECT 1;\n-- local unsaved\n");
  fs.writeFileSync(file, "-- editable\nSELECT 1;\n-- from mcp\n");
  await expect(page.locator("#patch-pop")).toBeVisible();
  await expect(page.locator("#editor-content")).toContainText("from mcp");
  await expect(page.locator("#editor-content")).not.toContainText("local unsaved");
  await expect(page.locator("#patch-pop-body")).toContainText("+-- local unsaved");
  await expect(page.locator("#patch-pop-title")).toHaveText("File changed on disk");
  const disk = fs.readFileSync(file, "utf8");
  expect(disk).toContain("from mcp");
  expect(disk).not.toContain("local unsaved");

  await page.locator("#patch-pop-close").click();
  await expect(page.locator("#patch-pop")).toBeHidden();
  await page.locator("#patch-recall").click();
  await expect(page.locator("#patch-pop-body")).toContainText("local unsaved");
  await page.screenshot({ path: "/opt/cursor/artifacts/file-patch-modal.png" });
  await page.waitForTimeout(1800);
  expect(fs.readFileSync(file, "utf8")).not.toContain("local unsaved");
});

test("a second window picks up a saved edit without a patch", async ({ page, app }) => {
  const other = await page.context().newPage();
  await openApp(other, app);
  await replaceEditor(page, "-- editable\nSELECT 1;\n-- from window\n");
  await expect(other.locator("#editor-content")).toContainText("from window");
  await expect(other.locator("#patch-pop")).toBeHidden();
  await other.close();
});

test("a dirty window does not overwrite another window's save", async ({ page, app }) => {
  const other = await page.context().newPage();
  await openApp(other, app);
  await other.route("**/api/file", async (route) => {
    if (route.request().method() === "PUT") {
      await new Promise((resolve) => setTimeout(resolve, 2000));
    }
    await route.continue();
  });
  const file = path.join(app.workspace, "queries/edit.dql");
  await replaceEditor(other, "-- editable\nSELECT 1;\n-- other unsaved\n");
  await replaceEditor(page, "-- editable\nSELECT 1;\n-- from first\n");
  await expect(other.locator("#patch-pop")).toBeVisible();
  await expect(other.locator("#patch-pop-body")).toContainText("other unsaved");
  await expect(other.locator("#editor-content")).toContainText("from first");
  const disk = fs.readFileSync(file, "utf8");
  expect(disk).toContain("from first");
  expect(disk).not.toContain("other unsaved");
  await other.close();
});

test("lists each open web window on the title bar", async ({ page, app }) => {
  await expect(page.locator(".presence-icon")).toHaveCount(1);
  await expect(page.locator(".presence-icon.is-self")).toHaveCount(1);
  await expect(page.locator(".presence-icon")).toHaveText("W");
  const other = await page.context().newPage();
  await openApp(other, app);
  await expect(page.locator(".presence-icon")).toHaveCount(2);
  await expect(other.locator(".presence-icon")).toHaveCount(2);
  await expect(page.locator(".presence-icon.is-self")).toHaveCount(1);
  await expect(other.locator(".presence-icon.is-self")).toHaveCount(1);
  await expect(page.locator("#presence")).toHaveAttribute("aria-label", "Connected: Web, Web");
  await page.screenshot({ path: "/opt/cursor/artifacts/presence-two-web.png" });
  await other.close();
  await expect(page.locator(".presence-icon")).toHaveCount(1);
});

test("shows web and mcp clients as profile icons", async ({ page }) => {
  await page.route("**/api/presence", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        clients: [
          { id: "web-window-a", kind: "web", color: "#0067a5" },
          { id: "web-window-b", kind: "web", color: "#c2410c" },
          { id: "mcp", kind: "mcp", color: "#6a3d9a" },
        ],
      }),
    });
  });
  await page.reload();
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  await expect(page.locator(".presence-icon")).toHaveText(["W", "W", "M"]);
  await expect(page.locator("#presence")).toHaveAttribute("aria-label", "Connected: Web, Web, MCP");
  await page.screenshot({ path: "/opt/cursor/artifacts/presence-web-mcp.png" });
});
