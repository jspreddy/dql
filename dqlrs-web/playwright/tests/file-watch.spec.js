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
  await expect(page).toHaveTitle("dqlrs: W1");
  await expect(page.locator(".presence-icon")).toHaveCount(1);
  await expect(page.locator(".presence-icon.is-self")).toHaveText("W1");
  await expect(page.locator("header.top")).toHaveAttribute("data-identity", "W1");
  const other = await page.context().newPage();
  await openApp(other, app);
  await expect(page.locator(".brand-mark > .presence-icon.is-self")).toHaveText("W1");
  await expect(page.locator("#presence .presence-icon")).toHaveText("W2");
  await expect(other.locator(".brand-mark > .presence-icon.is-self")).toHaveText("W2");
  await expect(other.locator("#presence .presence-icon")).toHaveText("W1");
  await expect(page.locator(".brand")).toHaveText("DQLRS Web");
  await expect(other).toHaveTitle("dqlrs: W2");
  await expect(page.locator("#presence")).toHaveAttribute("aria-label", "Connected: W1, W2");
  await expect(other.locator("#presence")).toHaveAttribute("aria-label", "Connected: W1, W2");
  const bar = await page.locator("header.top").evaluate((el) => getComputedStyle(el).backgroundColor);
  const mark = await page.locator(".brand-mark").evaluate((el) => getComputedStyle(el).backgroundColor);
  const mine = await page.locator(".presence-icon.is-self").evaluate((el) => getComputedStyle(el).backgroundColor);
  const otherBar = await other.locator("header.top").evaluate((el) => getComputedStyle(el).backgroundColor);
  const otherMark = await other.locator(".brand-mark").evaluate((el) => getComputedStyle(el).backgroundColor);
  const otherMine = await other.locator(".presence-icon.is-self").evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(mark).toBe(mine);
  expect(otherMark).toBe(otherMine);
  expect(bar).toBe(otherBar);
  expect(mark).not.toBe(bar);
  expect(mark).not.toBe(otherMark);
  const slash = await page.locator(".brand-mark").evaluate((el) => {
    const rect = el.getBoundingClientRect();
    const style = getComputedStyle(el);
    const shift = rect.height * 0.75;
    const owns = (x, y) => el.contains(document.elementFromPoint(x, y));
    return {
      height: rect.height,
      paddingRight: parseFloat(style.paddingRight),
      top: owns(rect.right - 2, rect.top + 2),
      bottomCut: owns(rect.right - 2, rect.bottom - 2),
      bottomKept: owns(rect.right - shift - 4, rect.bottom - 2),
    };
  });
  expect(slash.top).toBe(true);
  expect(slash.bottomCut).toBe(false);
  expect(slash.bottomKept).toBe(true);
  expect(slash.height * 0.75).toBeCloseTo(27, 0);
  expect(slash.paddingRight).toBeCloseTo(14 + 27 - 10, 0);
  await page.screenshot({ path: "/opt/cursor/artifacts/presence-two-web.png" });
  await other.screenshot({ path: "/opt/cursor/artifacts/presence-w2-titlebar.png" });
  const connected = ["2 clients connected", "This window is W1", "Web: W1, W2"];
  await page.locator(".presence-icon.is-self").hover();
  await expect(page.locator("#presence-tip")).toBeVisible();
  for (const line of connected) await expect(page.locator("#presence-tip")).toContainText(line);
  await page.screenshot({ path: "/opt/cursor/artifacts/presence-tooltip.png" });
  await page.locator("#presence .presence-icon").hover();
  await expect(page.locator("#presence-tip")).toBeVisible();
  for (const line of connected) await expect(page.locator("#presence-tip")).toContainText(line);
  await page.screenshot({ path: "/opt/cursor/artifacts/presence-tooltip-other.png" });
  await other.locator(".presence-icon.is-self").hover();
  await expect(other.locator("#presence-tip")).toContainText("This window is W2");
  await expect(other.locator("#presence-tip")).toContainText("2 clients connected");
  await expect(other.locator("#presence-tip")).toContainText("Web: W1, W2");
  await other.close();
  await expect(page.locator(".presence-icon")).toHaveCount(1);
  await expect(page.locator(".presence-icon")).toHaveText("W1");
  await expect(page).toHaveTitle("dqlrs: W1");
});

test("shows web and mcp clients as profile icons", async ({ page }) => {
  await page.route("**/api/presence", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        clients: [
          { id: "web-window-a", kind: "web", label: "W1", color: "#c2410c" },
          { id: "web-window-b", kind: "web", label: "W2", color: "#0067a5" },
          { id: "mcp", kind: "mcp", label: "M1", color: "#b45309" },
        ],
      }),
    });
  });
  await page.reload();
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  await expect(page.locator(".presence-icon")).toHaveText(["W1", "W2", "M1"]);
  await expect(page.locator("#presence")).toHaveAttribute("aria-label", "Connected: W1, W2, M1");
  await page.locator(".presence-icon").nth(2).hover();
  await expect(page.locator("#presence-tip")).toContainText("3 clients connected");
  await expect(page.locator("#presence-tip")).toContainText("Web: W1, W2");
  await expect(page.locator("#presence-tip")).toContainText("MCP: M1");
  await page.locator(".presence-icon").first().hover();
  await expect(page.locator("#presence-tip")).toContainText("3 clients connected");
  await expect(page.locator("#presence-tip")).toContainText("MCP: M1");
  await page.screenshot({ path: "/opt/cursor/artifacts/presence-web-mcp.png" });
});
