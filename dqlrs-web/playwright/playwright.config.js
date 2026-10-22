import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@playwright/test";

const here = path.dirname(fileURLToPath(import.meta.url));

// playwright test has no --slow-mo flag. just web-test-slow sets this.
const slowMo = Number(process.env.PLAYWRIGHT_SLOW_MO || 0);

const sharedUse = {
  viewport: { width: 1280, height: 800 },
  channel: "chrome",
  trace: "retain-on-failure",
  screenshot: "only-on-failure",
  ...(slowMo > 0 ? { launchOptions: { slowMo } } : {}),
};

export default defineConfig({
  testDir: "./tests",
  globalSetup: path.join(here, "support/global-setup.js"),
  timeout: 90_000,
  expect: { timeout: 20_000 },
  fullyParallel: false,
  workers: 1,
  reporter: "list",
  projects: [
    { name: "light", use: sharedUse },
    { name: "dark", use: sharedUse },
  ],
});
