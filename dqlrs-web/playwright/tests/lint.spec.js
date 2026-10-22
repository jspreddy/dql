import { test, expect } from "../support/app.js";
import { openApp, replaceEditor, themeColor } from "../support/helpers.js";

const BROKEN = "SELEKT * FROM shelf;";
const UNKNOWN = "SELECT * FROM lint_missing_shelf WHERE id = 'a';";
const CLEAN = "CREATE TABLE shelf (id STRING HASH KEY);\nSELECT * FROM shelf WHERE id = 'a';";

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

test("underlines a bad query, warns when the table is unknown, and clears a valid script", async ({ page }) => {
  await replaceEditor(page, BROKEN);
  const status = page.locator("#lint-status");
  await expect(status).toBeVisible();
  await expect(status).toHaveText("1 error");
  await expect(status).not.toHaveClass(/is-warning/);
  await expect(status).toHaveAttribute("title", /expected a DQL statement/);
  const mark = page.locator(".cm-lint-error");
  await expect(mark).toHaveCount(1);
  await expect(mark).toHaveAttribute("title", /expected a DQL statement/);
  await expect(page.locator("[data-testid='lint-gutter-error']")).toHaveCount(1);
  await expect(status).toHaveCSS("color", await themeColor(page, "--danger"));
  await expect(status).toHaveCSS("background-color", await themeColor(page, "--danger-wash"));
  await expect(mark).toHaveCSS("text-decoration-color", await themeColor(page, "--danger-bright"));
  await expect(page.locator("[data-testid='lint-gutter-error']")).toHaveCSS(
    "color",
    await themeColor(page, "--danger-bright"),
  );

  await status.click();
  await expect(page.locator("#editor-content")).toBeFocused();
  const cursorLine = await page.evaluate(() => {
    const cursor = document.querySelector(".cm-cursor");
    if (!cursor) return "";
    const box = cursor.getBoundingClientRect();
    const line = [...document.querySelectorAll(".cm-content .cm-line")].find((el) => {
      const rect = el.getBoundingClientRect();
      return box.top < rect.bottom && box.bottom > rect.top;
    });
    return line ? line.textContent : "";
  });
  expect(cursorLine).toContain("SELEKT");

  await replaceEditor(page, UNKNOWN);
  await expect(status).toBeVisible();
  await expect(status).toHaveText("1 warning");
  await expect(status).toHaveClass(/is-warning/);
  await expect(status).toHaveAttribute("title", /no cached definition/);
  await expect(page.locator(".cm-lint-error")).toHaveCount(0);
  const warning = page.locator(".cm-lint-warning");
  await expect(warning).toHaveCount(1);
  await expect(page.locator("[data-testid='lint-gutter-warning']")).toHaveCount(1);
  await expect(status).toHaveCSS("color", await themeColor(page, "--amber-ink"));
  await expect(status).toHaveCSS("background-color", await themeColor(page, "--amber-bg"));
  await expect(warning).toHaveCSS("text-decoration-color", await themeColor(page, "--amber"));

  await replaceEditor(page, CLEAN);
  await expect(status).toBeHidden();
  await expect(page.locator(".cm-lint-error")).toHaveCount(0);
  await expect(page.locator(".cm-lint-warning")).toHaveCount(0);
  await expect(page.locator("[data-testid='lint-gutter-error']")).toHaveCount(0);
  await expect(page.locator("[data-testid='lint-gutter-warning']")).toHaveCount(0);
});
