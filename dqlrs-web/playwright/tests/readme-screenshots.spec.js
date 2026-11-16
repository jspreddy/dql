import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { examples, openQueries, runStatement, seedCosmos, settle } from "../support/cosmos.js";

// Copy them over as described in screenshots/README.md (drop the readme-web- prefix).

test.use({
  workspaceSource: examples,
  viewport: { width: 1440, height: 900 },
});

test("README web shots: query, explain, tables", async ({ page, app }, testInfo) => {
  // dark: screenshots/web/ and the banner panel. light: kept for anyone who wants light shots.
  const shot = (name) =>
    playwrightArtifact(`readme-web-${name}${testInfo.project.name === "dark" ? "-dark" : ""}.png`);
  // Leave sample_cosmos in place: it is the same table `just web` users load from the cosmos example.
  seedCosmos(app.binary);

  await openQueries(page, app);
  await runStatement(page, "SELECT name, kind, class, radius_km FROM sample_cosmos", { above: 8 });
  await expect(page.locator("#result-table")).toContainText("Jupiter");
  await settle(page);
  await page.screenshot({ path: shot("query") });

  await runStatement(page, "EXPLAIN SELECT * FROM sample_cosmos", { above: 8 });
  await expect(page.getByRole("term").filter({ hasText: "key condition expression" }).last()).toBeVisible();
  await settle(page);
  await page.screenshot({ path: shot("explain") });

  await page.locator("#mode-tables").click();
  await page.locator("#table-list").getByText("sample_cosmos", { exact: true }).click();
  await expect(page.locator("#tables-view")).toContainText("PLANET#earth");
  await settle(page);
  await page.screenshot({ path: shot("tables") });
});
