import fs from "node:fs";

import { playwrightArtifacts, screenshotsEnabled } from "./artifacts.js";

export default function globalSetup() {
  // The pager test writes a JSON record here even when review shots are off.
  // `--with-screenshots` replaces the previous review set. A normal run keeps it.
  if (screenshotsEnabled()) fs.rmSync(playwrightArtifacts, { recursive: true, force: true });
  fs.mkdirSync(playwrightArtifacts, { recursive: true });
}
