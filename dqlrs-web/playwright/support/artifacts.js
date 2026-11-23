import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));

// support/ -> playwright/ -> dqlrs-web/ -> repository root
export const repoRoot = path.resolve(here, "../../..");
export const playwrightArtifacts = path.join(repoRoot, ".playwright-artifacts");

export function playwrightArtifact(name) {
  return path.join(playwrightArtifacts, name);
}

// `just web-test -- --with-screenshots` sets this. The saved shots are for review;
// the suite does not compare them.
export function screenshotsEnabled() {
  return process.env.PLAYWRIGHT_SCREENSHOTS === "1";
}

export function skipArtifactShot(options) {
  if (screenshotsEnabled()) return false;
  const filePath = options && options.path;
  if (!filePath) return false;
  const resolved = path.resolve(String(filePath));
  return resolved === playwrightArtifacts || resolved.startsWith(`${playwrightArtifacts}${path.sep}`);
}
