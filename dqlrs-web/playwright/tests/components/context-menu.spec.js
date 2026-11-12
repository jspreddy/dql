import { test, expect } from "../../support/app.js";
import { playwrightArtifact } from "../../support/artifacts.js";
import { openComponentHarness } from "../../support/components.js";

// context-menu.js: a menu of buttons placed at a point and kept inside the window.

test.beforeEach(async ({ page, app }, testInfo) => {
  await openComponentHarness(page, app, testInfo);
  await page.evaluate(async () => {
    const { createContextMenu } = await import("/context-menu.js");
    window.ran = [];
    window.menu = createContextMenu(null, {
      ids: { Run: "menu-run", Delete: "menu-delete" },
      icon: (name) => {
        const mark = document.createElement("span");
        mark.className = "test-icon";
        mark.textContent = name === "forward" ? "»" : "";
        return mark;
      },
    });
    window.items = [
      { label: "Run", icon: "forward", danger: true, run: () => window.ran.push("Run") },
      "-",
      { label: "Rename", run: () => window.ran.push("Rename") },
      { label: "Delete", danger: true, run: () => window.ran.push("Delete") },
    ];
  });
});

const menu = (page) => page.locator('[role="menu"]');

test("shows items, separators, ids, icons, and runs the chosen item", async ({ page }, testInfo) => {
  await page.evaluate(() => window.menu.show(40, 50, window.items));
  await expect(menu(page)).toBeVisible();
  await expect(menu(page).locator('[role="menuitem"]')).toHaveText(["»Run", "Rename", "Delete"]);
  await expect(menu(page).locator("hr")).toHaveCount(1);
  await expect(page.locator("#menu-run")).toHaveClass(/danger/);
  await expect(page.locator("#menu-run .test-icon")).toHaveText("»");
  await expect(page.locator("#menu-delete")).toHaveClass(/danger/);
  const box = await menu(page).boundingBox();
  expect(Math.round(box.x)).toBe(40);
  expect(Math.round(box.y)).toBe(50);
  await page.screenshot({ path: playwrightArtifact(`component-context-menu-${testInfo.project.name}.png`) });

  await menu(page).getByRole("menuitem", { name: "Rename" }).click();
  await expect(menu(page)).toBeHidden();
  expect(await page.evaluate(() => window.ran)).toEqual(["Rename"]);
});

test("stays inside the window near an edge", async ({ page }) => {
  const viewport = page.viewportSize();
  await page.evaluate(([x, y]) => window.menu.show(x, y, window.items), [viewport.width - 2, viewport.height - 2]);
  const box = await menu(page).boundingBox();
  expect(box.x + box.width).toBeLessThanOrEqual(viewport.width - 8 + 1);
  expect(box.y + box.height).toBeLessThanOrEqual(viewport.height - 8 + 1);
  expect(box.x).toBeGreaterThanOrEqual(4);
});

test("closes on a click outside and on Escape without running anything", async ({ page }) => {
  await page.evaluate(() => window.menu.show(40, 50, window.items));
  await expect(menu(page)).toBeVisible();
  await page.mouse.click(600, 600);
  await expect(menu(page)).toBeHidden();
  await page.evaluate(() => window.menu.show(40, 50, window.items));
  await expect(menu(page)).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(menu(page)).toBeHidden();
  expect(await page.evaluate(() => window.ran)).toEqual([]);
});
