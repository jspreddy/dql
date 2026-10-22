import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));

// support/ -> playwright/ -> dqlrs-web/ -> repository root
export const repoRoot = path.resolve(here, "../../..");
export const playwrightArtifacts = path.join(repoRoot, ".playwright-artifacts");

export function playwrightArtifact(name) {
  return path.join(playwrightArtifacts, name);
}
