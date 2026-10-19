import { defineConfig } from "@playwright/test";

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
