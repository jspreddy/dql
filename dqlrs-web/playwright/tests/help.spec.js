import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { openApp, replaceEditor, themeColor } from "../support/helpers.js";

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

test("renders HELP SELECT with syntax highlighting", async ({ page }, testInfo) => {
  await replaceEditor(page, "HELP SELECT");
  await page.locator("#run").click();
  const card = page.locator("[data-testid='help-card']");
  await expect(card).toBeVisible();
  await expect(card.locator(".help-title")).toHaveText("HELP select");
  const shell = card.locator("xpath=ancestor::*[contains(@class,'result-shell')]");
  await expect(shell.locator("[data-testid='result-query-toggle']")).toHaveCount(0);
  await expect(shell.locator("[data-testid='result-card-query']")).toHaveCount(0);
  const keyword = card.locator(".help-code .tok-keyword", { hasText: /^SELECT$/ }).first();
  await expect(keyword).toBeVisible();
  await expect(keyword).toHaveCSS("color", await themeColor(page, "--teal-ink"));
  const string = card.locator(".help-code .tok-string").first();
  await expect(string).toContainText("'a'");
  await expect(string).toHaveCSS("color", await themeColor(page, "--rose"));
  const examples = card.locator("[data-testid='help-code']").nth(1);
  await expect(examples).toContainText("CREATE TABLE");
  await page.locator("#result-body").screenshot({
    path: playwrightArtifact(`help-select-${testInfo.project.name}.png`),
  });
  await examples.scrollIntoViewIfNeeded();
  await page.locator("#result-body").screenshot({
    path: playwrightArtifact(`help-select-examples-${testInfo.project.name}.png`),
  });
});

test("renders the HELP index as a command list", async ({ page }, testInfo) => {
  await replaceEditor(page, "HELP");
  await page.locator("#run").click();
  const card = page.locator("[data-testid='help-card']");
  await expect(card.locator(".help-title")).toHaveText("Available commands");
  const select = card.locator(".help-entry .tok-keyword", { hasText: /^select$/ });
  await expect(select).toBeVisible();
  await expect(select).toHaveCSS("color", await themeColor(page, "--teal-ink"));
  await expect(card).toContainText("Query items from a table or index");
  await page.locator("#result-body").screenshot({
    path: playwrightArtifact(`help-index-${testInfo.project.name}.png`),
  });
});
