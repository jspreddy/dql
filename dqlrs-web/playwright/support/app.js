import { test as base, expect } from "@playwright/test";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(here, "../..");
const fixtureRoot = path.resolve(here, "../fixtures/workspace");

function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
    server.on("error", reject);
  });
}

function dqlrsBinary() {
  const candidates = [
    process.env.DQLRS_BIN,
    path.resolve(webRoot, "../rust-impl/target/debug/dqlrs"),
    path.resolve(webRoot, "../rust-impl/target/release/dqlrs"),
  ].filter(Boolean);
  return candidates.find((candidate) => fs.existsSync(candidate)) || "";
}

function dropTable(binary, name) {
  if (!binary) return;
  const result = spawnSync(
    binary,
    ["-c", `DROP TABLE IF EXISTS ${name}`, "-H", "localhost", "-p", "8000"],
    {
      encoding: "utf8",
      timeout: 20_000,
      env: {
        ...process.env,
        AWS_ACCESS_KEY_ID: "fakeid",
        AWS_SECRET_ACCESS_KEY: "fakekey",
        AWS_REGION: "us-west-1",
        AWS_EC2_METADATA_DISABLED: "true",
      },
    },
  );
  if (result.status !== 0) {
    console.warn(`DROP ${name} failed`, result.stderr || result.stdout || result.error);
  }
}

async function waitForServer(baseURL, child, log) {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`server exited ${child.exitCode}\n${log()}`);
    }
    try {
      const response = await fetch(`${baseURL}/api/config`);
      if (response.ok) return;
    } catch {
      /* retry until the port accepts connections */
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`server did not start\n${log()}`);
}

function stopChild(child) {
  return new Promise((resolve) => {
    if (child.exitCode !== null) {
      resolve();
      return;
    }
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      resolve();
    }, 4_000);
    child.once("exit", () => {
      clearTimeout(timer);
      resolve();
    });
    child.kill("SIGTERM");
  });
}

export const test = base.extend({
  app: async ({}, use) => {
    const workspace = fs.mkdtempSync(path.join(os.tmpdir(), "dqlrs-web-"));
    fs.cpSync(fixtureRoot, workspace, { recursive: true });
    const port = await freePort();
    const binary = dqlrsBinary();
    const chunks = [];
    const child = spawn("python3", ["server.py", "--dir", workspace, "--port", String(port), "--host", "127.0.0.1"], {
      cwd: webRoot,
      env: {
        ...process.env,
        PYTHONUNBUFFERED: "1",
        AWS_ACCESS_KEY_ID: "fakeid",
        AWS_SECRET_ACCESS_KEY: "fakekey",
        AWS_REGION: "us-west-1",
        AWS_EC2_METADATA_DISABLED: "true",
        DQL_HOST: "localhost",
        DQL_PORT: "8000",
        ...(binary ? { DQLRS_BIN: binary } : {}),
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    child.stdout.on("data", (chunk) => chunks.push(chunk));
    child.stderr.on("data", (chunk) => chunks.push(chunk));
    const baseURL = `http://127.0.0.1:${port}`;
    const tables = [];
    try {
      await waitForServer(baseURL, child, () => Buffer.concat(chunks).toString("utf8"));
      await use({
        baseURL,
        workspace,
        binary,
        trackTable(name) {
          tables.push(name);
        },
      });
    } finally {
      for (const name of tables) dropTable(binary, name);
      await stopChild(child);
      fs.rmSync(workspace, { recursive: true, force: true });
    }
  },
});

export { expect };
