import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { openApp, replaceEditor } from "../support/helpers.js";

// Tooltips wait 100, 200, or 400 ms before they show, by how much the tip matters and how obvious
// the labelled item is (dqlrs-web/DESIGN.md, Tooltips). The delay is measured in the page: from
// the pointer reaching the item to the tip appearing.

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

/** Start a clock in the page: it stops when #lint-tip appears. */
async function armTipClock(page) {
  await page.evaluate(() => {
    const tip = document.querySelector("#lint-tip");
    window.__tipShown = null;
    window.__tipStart = null;
    document.addEventListener("pointerover", () => {
      if (window.__tipStart == null) window.__tipStart = performance.now();
    }, { capture: true, once: true });
    const observer = new MutationObserver(() => {
      if (!tip.hidden && window.__tipShown == null) {
        window.__tipShown = performance.now();
        observer.disconnect();
      }
    });
    observer.observe(tip, { attributes: true, attributeFilter: ["hidden"] });
  });
}

async function tipDelay(page, locator) {
  await page.mouse.move(2, 790);
  await expect(page.locator("#lint-tip")).toBeHidden();
  // Let the warm hand-off window from the last tip pass.
  await page.waitForTimeout(350);
  await armTipClock(page);
  await locator.hover();
  await expect(page.locator("#lint-tip")).toBeVisible();
  return page.evaluate(() => window.__tipShown - window.__tipStart);
}

test("waits 100, 200, or 400 ms by how much the tip matters", async ({ page }, testInfo) => {
  // fast: a lint message is only readable in the tip.
  await replaceEditor(page, "SELEKT * FROM shelf;");
  await expect(page.locator("[data-testid='lint-gutter-error']")).toHaveCount(1);
  const fast = await tipDelay(page, page.locator(".cm-line", { hasText: "SELEKT" }));
  await expect(page.locator("#lint-tip")).toHaveText(/expected a DQL statement/);
  // medium: an icon whose meaning is not obvious.
  const medium = await tipDelay(page, page.locator("#collapse-tree"));
  await expect(page.locator("#lint-tip")).toHaveText("Collapse all folders");
  // slow: a universal icon (the theme's sun or moon).
  const slow = await tipDelay(page, page.locator("#theme-toggle"));
  await page.screenshot({ path: playwrightArtifact(`tooltip-slow-${testInfo.project.name}.png`) });

  expect(fast).toBeGreaterThanOrEqual(95);
  expect(fast).toBeLessThan(190);
  expect(medium).toBeGreaterThanOrEqual(195);
  expect(medium).toBeLessThan(330);
  expect(slow).toBeGreaterThanOrEqual(395);
  expect(slow).toBeLessThan(600);
});

test("data-tip-delay picks a tier, and an obvious item can opt back in", async ({ page }) => {
  await page.evaluate(() => document.querySelector("#theme-toggle").dataset.tipDelay = "fast");
  const fast = await tipDelay(page, page.locator("#theme-toggle"));
  expect(fast).toBeLessThan(190);
});

test("moving from a shown tip to the next shows it at once", async ({ page }) => {
  await tipDelay(page, page.locator("#collapse-tree"));
  const before = await page.evaluate(() => performance.now());
  await page.locator("#expand-tree").hover();
  await expect(page.locator("#lint-tip")).toHaveText("Expand all folders");
  const handoff = await page.evaluate((start) => performance.now() - start, before);
  expect(handoff).toBeLessThan(150);
});

test("leaving before the delay shows nothing", async ({ page }) => {
  await page.locator("#theme-toggle").hover();
  await page.waitForTimeout(150);
  await page.mouse.move(600, 500);
  await page.waitForTimeout(500);
  await expect(page.locator("#lint-tip")).toBeHidden();
});

test("items that say what they are have no tooltip", async ({ page }) => {
  // The endpoint chip, a labeled button, universal icons, and resize grips.
  for (const selector of [
    "#endpoint",
    "#run-file",
    "#settings-toggle",
    "#side-resize",
    "#results-resize",
    "#table-query-resize",
    "#table-query-height",
    "#schema-pop-close",
    "#run-pop-close",
    "#row-pop-close",
    "#patch-pop-close",
    "#mermaid-zoom-in",
    "#mermaid-zoom-out",
    "#mermaid-screen-close",
  ]) {
    await expect(page.locator(selector), selector).not.toHaveAttribute("data-tip");
  }
  await page.locator("#endpoint").hover();
  await page.waitForTimeout(500);
  await expect(page.locator("#lint-tip")).toBeHidden();
  // Controls whose tip still adds something keep it.
  await expect(page.locator("#collapse-tree")).toHaveAttribute("data-tip", "Collapse all folders");
  await expect(page.locator("#new-file")).toHaveAttribute("data-tip", "New .dql file");
});
