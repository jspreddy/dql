import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { examples, openQueries, runStatement, seedCosmos, settle } from "../support/cosmos.js";
import { node, reveal } from "../support/helpers.js";

// One screenshot of each main screen, in light and dark, for design review (dqlrs-web/DESIGN.md).

test.use({ workspaceSource: examples, viewport: { width: 1440, height: 900 } });

test("design review shots of every main screen", async ({ page, app }, testInfo) => {
  const shot = async (name) => {
    await settle(page);
    await page.screenshot({ path: playwrightArtifact(`design-${name}-${testInfo.project.name}.png`) });
  };
  seedCosmos(app.binary);

  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).not.toHaveText("No file open");
  await shot("01-first-load");

  await openQueries(page, app);
  await runStatement(page, "SELECT name, kind, class, radius_km FROM sample_cosmos", { above: 8 });
  await expect(page.locator("#result-table")).toContainText("Jupiter");
  await shot("02-query-result");

  await page.locator("#result-tab-history").click();
  await shot("03-history");
  await page.locator("#result-tab-lint").click();
  await shot("04-lint");
  await page.locator("#result-tab-result").click();

  await page.locator("#result-table tbody tr").first().dblclick();
  if (await page.locator("#row-pop").isVisible()) {
    await shot("05-row-dialog");
    await page.keyboard.press("Escape");
  }

  await page.locator("#settings-toggle").click();
  await expect(page.locator("#settings-pop")).toBeVisible();
  await shot("06-settings");
  await page.keyboard.press("Escape");

  await page.locator("#endpoint").click();
  await shot("07-endpoint");
  await page.keyboard.press("Escape");

  await page.locator("#title-search").fill("cosmos");
  await expect(page.locator("#title-search-list")).toBeVisible();
  await shot("08-search");
  await page.keyboard.press("Escape");
  await page.locator("#title-search").fill("");

  await node(page, "cosmos/20-queries.dql").click({ button: "right" });
  await expect(page.locator("#file-menu")).toBeVisible();
  await shot("09-file-menu");
  await page.locator("#file-menu").getByText("Run file", { exact: false }).first().click();
  await expect(page.locator("#run-pop")).toBeVisible();
  await shot("10-run-dialog");
  await page.locator("#run-pop-cancel").click();

  await page.locator("#new-file").click();
  await expect(page.locator("#prompt-pop")).toBeVisible();
  await shot("11-prompt-dialog");
  await page.keyboard.press("Escape");

  await page.locator('.file-filter[data-type="md"]').click();
  await page.locator('.file-filter[data-type="csv"]').click();
  await reveal(page, "cosmos/README.md");
  await node(page, "cosmos/README.md").click();
  await expect(page.locator("#doc-view")).toBeVisible();
  await shot("12-markdown");
  await reveal(page, "local-and-temp-tables/sample-users.csv");
  await node(page, "local-and-temp-tables/sample-users.csv").click();
  await expect(page.locator("#file-rows")).toBeVisible();
  await shot("13-csv");

  await page.locator("#mode-tables").click();
  await page.locator("#table-list").getByText("sample_cosmos", { exact: true }).click();
  await expect(page.locator("#tables-view")).toContainText("PLANET#earth");
  await shot("14-tables");
  await page.locator("#rows-schema").click();
  await expect(page.locator("#schema-pop")).toBeVisible();
  await shot("15-schema-dialog");
  await page.keyboard.press("Escape");

  await page.setViewportSize({ width: 820, height: 640 });
  await shot("16-tables-narrow");
  await page.locator("#mode-query").click();
  await node(page, "cosmos/20-queries.dql").click();
  await shot("17-query-narrow");
});
