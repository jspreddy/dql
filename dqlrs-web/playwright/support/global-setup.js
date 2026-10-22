import fs from "node:fs";

import { playwrightArtifacts } from "./artifacts.js";

export default function globalSetup() {
  fs.rmSync(playwrightArtifacts, { recursive: true, force: true });
  fs.mkdirSync(playwrightArtifacts, { recursive: true });
}
