import fs from "node:fs";

import { playwrightArtifacts, screenshotsEnabled } from "./artifacts.js";

export default function globalSetup() {
  // A normal run leaves the last review set in place. `--with-screenshots` replaces it.
  if (!screenshotsEnabled()) return;
  fs.rmSync(playwrightArtifacts, { recursive: true, force: true });
  fs.mkdirSync(playwrightArtifacts, { recursive: true });
}
