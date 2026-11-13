import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { node, reveal } from "../support/helpers.js";

// Copy them over as described in screenshots/README.md (drop the readme-web- prefix).

const here = path.dirname(fileURLToPath(import.meta.url));
const examples = path.resolve(here, "../../examples");

test.use({
  workspaceSource: examples,
  viewport: { width: 1440, height: 900 },
});

function seedCosmos(binary) {
  // `file` reads one path per call. Passing the DQL to -c does not work: it starts with a `--` comment.
  // dqlrs keeps its cache in <cwd>/.dqlrs-cache, so run it from a temp folder, not the repository.
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), "dqlrs-seed-"));
  try {
    for (const name of ["00-table.dql", "10-bodies.dql"]) {
      const result = spawnSync(
        binary,
        ["-H", "localhost", "-p", "8000", "-c", `file ${path.join(examples, "cosmos", name)}`],
        {
          cwd,
          encoding: "utf8",
          timeout: 60_000,
          env: {
            ...process.env,
            AWS_ACCESS_KEY_ID: "fakeid",
            AWS_SECRET_ACCESS_KEY: "fakekey",
            AWS_REGION: "us-west-1",
            AWS_EC2_METADATA_DISABLED: "true",
          },
        },
      );
      expect(result.status, result.stderr || result.stdout).toBe(0);
    }
  } finally {
    fs.rmSync(cwd, { recursive: true, force: true });
  }
}

async function openQueries(page, app) {
  await page.goto(`${app.baseURL}/`);
  await reveal(page, "cosmos/20-queries.dql");
  await node(page, "cosmos/20-queries.dql").click();
  await expect(page.locator("#file-name")).toHaveText("20-queries.dql");
}

// Scroll the statement's first line near the top of the editor, put the cursor in it, and run it.
async function runStatement(page, firstLine, { above = 1 } = {}) {
  const line = page.locator(".cm-line", { hasText: firstLine }).first();
  await line.scrollIntoViewIfNeeded();
  await line.evaluate((el, rows) => {
    const scroller = el.closest(".cm-scroller");
    scroller.scrollTop += el.getBoundingClientRect().top - scroller.getBoundingClientRect().top - rows * el.offsetHeight;
  }, above);
  await line.click();
  await page.locator("#run").click();
  await expect(page.locator("#run")).toBeEnabled();
}

async function settle(page) {
  await page.mouse.move(0, 0);
  await page.evaluate(() => document.activeElement?.blur());
  await page.waitForTimeout(300);
}

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
