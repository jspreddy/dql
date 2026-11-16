import { test, expect } from "../../support/app.js";
import { playwrightArtifact } from "../../support/artifacts.js";
import { openComponentHarness } from "../../support/components.js";
import { themeColor } from "../../support/helpers.js";

// components.css: the button, the icon button, and the dialog card.

test.beforeEach(async ({ page, app }, testInfo) => {
  await openComponentHarness(page, app, testInfo);
  await page.evaluate(() => {
    document.querySelector("#root").innerHTML = `
      <div class="dialog-card" id="card">
        <h2 class="dialog-title">Delete notes.dql?</h2>
        <p style="margin: 8px 0 16px">This cannot be undone.</p>
        <div class="dialog-actions">
          <button type="button" class="icon-btn" id="close" aria-label="Close">
            <svg class="icon" width="12" height="12" viewBox="0 0 24 24" aria-hidden="true"><use href="/vendor/icons/close.svg#icon"></use></svg>
          </button>
          <button type="button" class="btn" id="quiet">Cancel</button>
          <button type="button" class="btn" id="off" disabled>Disabled</button>
          <button type="button" class="btn btn-primary" id="primary">Create</button>
          <button type="button" class="btn btn-danger" id="danger">Delete</button>
        </div>
      </div>`;
  });
});

test("buttons take their fills from the theme and share one size", async ({ page }, testInfo) => {
  await expect(page.locator("#quiet")).toHaveCSS("background-color", await themeColor(page, "--chip"));
  await expect(page.locator("#primary")).toHaveCSS("background-color", await themeColor(page, "--teal"));
  await expect(page.locator("#primary")).toHaveCSS("color", await themeColor(page, "--on-fill"));
  await expect(page.locator("#danger")).toHaveCSS("background-color", await themeColor(page, "--danger-fill"));
  for (const id of ["quiet", "off", "primary", "danger"]) {
    await expect(page.locator(`#${id}`)).toHaveCSS("height", "32px");
    await expect(page.locator(`#${id}`)).toHaveCSS("border-radius", "8px");
  }
  await expect(page.locator("#off")).toHaveCSS("opacity", "0.6");
  await expect(page.locator("#close")).toHaveCSS("width", "28px");
  await expect(page.locator("#close")).toHaveCSS("height", "28px");
  await expect(page.locator("#card")).toHaveCSS("background-color", await themeColor(page, "--paper"));
  await expect(page.locator("#card")).toHaveCSS("border-radius", "14px");
  await page.screenshot({ path: playwrightArtifact(`component-buttons-${testInfo.project.name}.png`) });
});

test("keyboard focus draws the same ring on every button", async ({ page }) => {
  await page.keyboard.press("Tab");
  await expect(page.locator("#close")).toBeFocused();
  await expect(page.locator("#close")).toHaveCSS("outline-style", "solid");
  await expect(page.locator("#close")).toHaveCSS("outline-width", "2px");
  await expect(page.locator("#close")).toHaveCSS("outline-color", await themeColor(page, "--teal"));
  await page.keyboard.press("Tab");
  await expect(page.locator("#quiet")).toBeFocused();
  await expect(page.locator("#quiet")).toHaveCSS("outline-color", await themeColor(page, "--teal"));
  // The disabled button is skipped.
  await page.keyboard.press("Tab");
  await expect(page.locator("#primary")).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.locator("#danger")).toBeFocused();
  await expect(page.locator("#danger")).toHaveCSS("outline-color", await themeColor(page, "--danger-outline"));
});
