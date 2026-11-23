import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@playwright/test";

const here = path.dirname(fileURLToPath(import.meta.url));

// playwright test has no --slow-mo flag. just web-test-slow sets this.
const slowMo = Number(process.env.PLAYWRIGHT_SLOW_MO || 0);

// WSL's Chrome hangs Page.captureScreenshot on the larger design shots unless GPU
// and /dev/shm are taken out of the path. Other platforms keep the default launch.
function runningOnWsl() {
  if (process.env.WSL_DISTRO_NAME || process.env.WSL_INTEROP) return true;
  try {
    return /microsoft|wsl/i.test(fs.readFileSync("/proc/sys/kernel/osrelease", "utf8"));
  } catch {
    return false;
  }
}

const launchOptions = {
  ...(runningOnWsl() ? { args: ["--disable-gpu", "--disable-dev-shm-usage"] } : {}),
  ...(slowMo > 0 ? { slowMo } : {}),
};

const sharedUse = {
  viewport: { width: 1280, height: 800 },
  channel: "chrome",
  trace: "retain-on-failure",
  screenshot: "only-on-failure",
  ...(Object.keys(launchOptions).length ? { launchOptions } : {}),
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
