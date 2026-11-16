import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect } from "@playwright/test";
import { node, reveal } from "./helpers.js";

// The cosmos example: seed its table, open its queries, and run a statement. Shared by the specs
// that take screenshots of the app with real data.

const here = path.dirname(fileURLToPath(import.meta.url));
export const examples = path.resolve(here, "../../examples");

export function seedCosmos(binary) {
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

export async function openQueries(page, app) {
  await page.goto(`${app.baseURL}/`);
  await reveal(page, "cosmos/20-queries.dql");
  await node(page, "cosmos/20-queries.dql").click();
  await expect(page.locator("#file-name")).toHaveText("20-queries.dql");
}

// Scroll the statement's first line near the top of the editor, put the cursor in it, and run it.
export async function runStatement(page, firstLine, { above = 1 } = {}) {
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

export async function settle(page) {
  await page.mouse.move(0, 0);
  await page.evaluate(() => document.activeElement?.blur());
  await page.waitForTimeout(300);
}
