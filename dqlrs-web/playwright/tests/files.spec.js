import fs from "node:fs";
import path from "node:path";
import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { handleDialogs, node, openApp, pressMod, replaceEditor, themeColor } from "../support/helpers.js";

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

test("resizes the left nav from the edge handle", async ({ page }) => {
  const side = page.locator("#side");
  const handle = page.locator("#side-resize");
  await expect(handle).toBeVisible();
  const before = await side.boundingBox();
  const grip = await handle.boundingBox();
  const y = grip.y + grip.height / 2;
  await page.mouse.move(grip.x + grip.width / 2, y);
  await page.mouse.down();
  await page.mouse.move(grip.x + grip.width / 2 + 90, y, { steps: 8 });
  const wider = await side.boundingBox();
  expect(wider.width).toBeGreaterThan(before.width + 60);

  await expect.poll(() => page.evaluate(() => localStorage.getItem("dqlrs-web.side-width")), {
    timeout: 1_000,
  }).not.toBeNull();
  const storedWhileHeld = Number(await page.evaluate(() => localStorage.getItem("dqlrs-web.side-width")));
  expect(Math.abs(storedWhileHeld - wider.width)).toBeLessThan(3);

  await page.mouse.up();
  await page.reload();
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  const restored = await page.locator("#side").boundingBox();
  expect(Math.abs(restored.width - wider.width)).toBeLessThan(3);

  await handle.focus();
  await page.keyboard.press("ArrowLeft");
  const narrower = await page.locator("#side").boundingBox();
  expect(narrower.width).toBeLessThan(restored.width - 10);

  await handle.dblclick();
  await expect.poll(() => page.evaluate(() => localStorage.getItem("dqlrs-web.side-width"))).toBeNull();
  const reset = await page.locator("#side").boundingBox();
  expect(Math.abs(reset.width - 220)).toBeLessThan(8);
});

test("shows nested files, an empty folder, and opens a file", async ({ page }) => {
  await expect(node(page, "empty")).toBeVisible();
  await expect(node(page, "queries")).toBeVisible();
  await expect(node(page, "queries/edit.dql")).toBeVisible();
  await expect(node(page, "queries/read.dql")).toBeVisible();
  await expect(node(page, "queries/functions.dql")).toBeVisible();
  await expect(node(page, "queries/edit.dql")).toHaveClass(/selected/);

  await node(page, "queries").click();
  await expect(node(page, "queries/edit.dql")).toHaveCount(0);

  await node(page, "queries").click();
  await node(page, "queries/read.dql").click();
  await expect(page.locator("#file-name")).toHaveText("read.dql");
  await expect(node(page, "queries/read.dql")).toHaveClass(/selected/);
});

async function expectUnderline(tab, primary) {
  const line = await tab.evaluate((el) => {
    const style = getComputedStyle(el, "::after");
    return { height: style.height, background: style.backgroundColor, bottom: style.bottom };
  });
  expect(line.height).toBe("2px");
  expect(line.background).toBe(primary);
  expect(line.bottom).toBe("-1px");
}

test("shows csv and json files and opens them read only", async ({ page, app }, testInfo) => {
  fs.writeFileSync(path.join(app.workspace, "users.csv"), "id,name\na,ada\nb,grace\n");
  fs.writeFileSync(path.join(app.workspace, "cities.json"), "[{\"id\":\"a\",\"city\":\"nyc\"}]\n");
  fs.writeFileSync(path.join(app.workspace, "notes.txt"), "hidden\n");
  await page.reload();
  await expect(node(page, "users.csv")).toBeVisible();
  await expect(node(page, "cities.json")).toBeVisible();
  await expect(node(page, "notes.txt")).toHaveCount(0);
  await expect(node(page, "users.csv").locator(".file-kind")).toHaveText("csv");
  await expect(node(page, "cities.json").locator(".file-kind")).toHaveText("json");

  await node(page, "users.csv").click();
  await expect(page.locator("#file-name")).toHaveText("users.csv");
  await expect(page.locator("#editor-content")).toContainText("ada");
  await expect(page.locator("#editor-content")).toContainText("grace");
  await expect(page.locator("#editor-content")).toHaveAttribute("contenteditable", "false");
  await expect(page.locator("#run")).toBeHidden();
  await expect(page.locator("#run-file")).toBeHidden();
  await node(page, "users.csv").click({ button: "right" });
  await expect(page.locator("#menu-run-file")).toHaveCount(0);
  await page.locator("#file-name").click();
  await page.screenshot({ path: playwrightArtifact(`view-csv-${testInfo.project.name}.png`) });

  const before = fs.readFileSync(path.join(app.workspace, "users.csv"), "utf8");
  await page.locator("#editor-content").click();
  await page.keyboard.insertText("nope");
  await expect(page.locator("#editor-content")).toContainText("ada");
  await expect(page.locator("#editor-content")).not.toContainText("nope");
  expect(fs.readFileSync(path.join(app.workspace, "users.csv"), "utf8")).toBe(before);

  await node(page, "cities.json").click();
  await expect(page.locator("#file-name")).toHaveText("cities.json");
  await expect(page.locator("#editor-content")).toContainText("nyc");
  await expect(page.locator("#editor-content")).toHaveAttribute("contenteditable", "false");
  await page.screenshot({ path: playwrightArtifact(`view-json-${testInfo.project.name}.png`) });

  await node(page, "queries/read.dql").click();
  await expect(page.locator("#file-name")).toHaveText("read.dql");
  await expect(page.locator("#editor-content")).toHaveAttribute("contenteditable", "true");
  await expect(page.locator("#run")).toBeEnabled();
});

test("marks the file open in the editor", async ({ page }, testInfo) => {
  const edit = node(page, "queries/edit.dql");
  const primary = await themeColor(page, "--teal");
  const tab = page.locator("#file-name");
  await expect(edit).toHaveClass(/is-open/);
  await expect(edit).toHaveAttribute("aria-current", "true");
  await expect(edit).toHaveCSS("border-right-width", "2px");
  await expect(edit).toHaveCSS("border-right-color", primary);
  await expect(tab).toHaveClass(/is-open/);
  await expect(tab).toHaveCSS("border-bottom-width", "0px");
  await expectUnderline(tab, primary);
  await expect(page.locator(".node.is-open")).toHaveCount(1);
  await expect(page.locator(".node-running")).toHaveCount(0);
  await page.screenshot({ path: playwrightArtifact(`open-file-mark-${testInfo.project.name}.png`) });

  await node(page, "queries/read.dql").click();
  const read = node(page, "queries/read.dql");
  await expect(page.locator("#file-name")).toHaveText("read.dql");
  await expect(tab).toHaveClass(/is-open/);
  await expectUnderline(tab, primary);
  await expect(read).toHaveClass(/is-open/);
  await expect(read).toHaveAttribute("aria-current", "true");
  await expect(read).toHaveCSS("border-right-width", "2px");
  await expect(read).toHaveCSS("border-right-color", primary);
  await expect(edit).not.toHaveClass(/is-open/);
  await expect(edit).toHaveCSS("border-right-width", "0px");
  await expect(edit).not.toHaveAttribute("aria-current", /.+/);
  await expect(page.locator(".node.is-open")).toHaveCount(1);
});

test("pulses a blue dot on the running file and clears it", async ({ page, app }, testInfo) => {
  const name = `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
  app.trackTable(name);
  let release = () => {};
  const held = new Promise((resolve) => {
    release = resolve;
  });
  let heldRun = false;
  await page.route("**/api/run", async (route) => {
    if (!heldRun) {
      heldRun = true;
      await held;
    }
    await route.continue();
  });
  await replaceEditor(page, `CREATE TABLE ${name} (id STRING HASH KEY);`);
  await pressMod(page, "A");
  await page.locator("#run").click();
  const file = node(page, "queries/edit.dql");
  const dot = file.locator(".node-running");
  await expect(dot).toBeVisible();
  await expect(page.locator(".node-running")).toHaveCount(1);
  await expect(dot).toHaveCSS("background-color", await themeColor(page, "--blue"));
  await expect(dot).toHaveCSS("animation-name", "node-running-pulse");
  await expect(file).toHaveCSS("border-right-width", "2px");
  await page.screenshot({ path: playwrightArtifact(`running-file-dot-${testInfo.project.name}.png`) });
  release();
  await expect(page.locator(".node-running")).toHaveCount(0);
  await expect(page.locator('[data-testid="run-status-ok"]')).toBeVisible();
  await expect(file).toHaveClass(/is-open/);
});

test("context menu on a file offers the file actions", async ({ page }) => {
  await node(page, "queries/edit.dql").click({ button: "right" });
  await expect(page.locator("#menu-new-file")).toBeVisible();
  await expect(page.locator("#menu-new-folder")).toBeVisible();
  await expect(page.locator("#menu-duplicate")).toBeVisible();
  await expect(page.locator("#menu-rename")).toBeVisible();
  await expect(page.locator("#menu-move")).toBeVisible();
  await expect(page.locator("#menu-delete")).toBeVisible();
  await expect(page.locator("#menu-clear-results")).toBeVisible();
  await expect(page.locator("#menu-clear-cache")).toHaveCount(0);
  await expect(page.locator("#menu-run-folder")).toHaveCount(0);
  await expect(page.locator("#menu-run-file")).toBeVisible();
  await expect(page.locator("#menu-run-file .ff-icon")).toBeVisible();
  await expect(page.locator("#menu-run-file")).toHaveCSS("color", await themeColor(page, "--danger"));
  await page.locator("#file-name").click();
  await expect(page.locator("#file-menu")).toBeHidden();
});

test("sidebar background menu creates a file and a folder at the workspace root", async ({ page }) => {
  const dialogs = handleDialogs(page, async (dialog) => {
    expect(dialog.type()).toBe("prompt");
    if (dialog.message().includes("file")) {
      expect(dialog.defaultValue()).toBe("queries/notes.dql");
      await dialog.accept("top.dql");
      return;
    }
    expect(dialog.defaultValue()).toBe("folder");
    await dialog.accept("extra");
  });

  await page.locator("#new-file").click();
  await expect(node(page, "top.dql")).toBeVisible();
  await expect(page.locator("#file-name")).toHaveText("top.dql");

  await page.locator(".side-head").click({ button: "right" });
  await expect(page.locator("#menu-new-folder")).toBeVisible();
  await expect(page.locator("#menu-clear-cache")).toHaveCount(0);
  await expect(page.locator("#menu-clear-results")).toHaveCount(0);
  await expect(page.locator("#menu-run-folder")).toHaveCount(0);
  await expect(page.locator("#menu-run-file")).toHaveCount(0);
  await expect(page.locator("#menu-duplicate")).toHaveCount(0);
  await page.locator("#menu-new-folder").click();
  await expect(node(page, "extra")).toBeVisible();
  await dialogs.stop();
});

test("runs the selected file from the context menu", async ({ page, app }, testInfo) => {
  const name = `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
  app.trackTable(name);
  const file = `run-${name}.dql`;
  const text = [
    `DROP TABLE IF EXISTS ${name};`,
    `CREATE TABLE ${name} (id STRING HASH KEY);`,
    `INSERT INTO ${name} (id) VALUES ('a');`,
    `SELECT * FROM ${name} WHERE id = 'a';`,
  ].join("\n");
  const error = await page.evaluate(async ({ path, text }) => {
    const headers = { "Content-Type": "application/json" };
    const created = await fetch("/api/file", { method: "POST", headers, body: JSON.stringify({ path }) });
    if (!created.ok) return await created.text();
    const saved = await fetch("/api/file", { method: "PUT", headers, body: JSON.stringify({ path, text }) });
    return saved.ok ? "" : await saved.text();
  }, { path: file, text });
  if (error) throw new Error(error);
  await page.reload();
  await node(page, "queries/read.dql").click();
  await expect(page.locator("#file-name")).toHaveText("read.dql");
  await node(page, file).click({ button: "right" });
  await expect(page.locator("#menu-run-file")).toBeVisible();
  await page.screenshot({ path: playwrightArtifact(`run-file-menu-${testInfo.project.name}.png`) });
  await page.locator("#menu-run-file").click();
  await expect(page.locator("#file-menu")).toBeHidden();
  await expect(page.locator("#file-name")).toHaveText(`run-${name}.dql`);
  await expect(node(page, file)).toHaveClass(/selected/);
  await expect(page.locator('[data-testid="run-status-ok"]')).toHaveCount(4);
  await expect(page.locator(".query-result").last()).toContainText("a");

  await node(page, file).click({ button: "right" });
  await page.locator("#menu-run-file").click();
  await expect(page.locator("#file-name")).toHaveText(`run-${name}.dql`);
  await expect(page.locator('[data-testid="run-status-ok"]')).toHaveCount(4);
});

test("runs an empty folder from the context menu", async ({ page }) => {
  await node(page, "empty").click({ button: "right" });
  await expect(page.locator("#menu-run-folder")).toBeVisible();
  await expect(page.locator("#menu-run-file")).toHaveCount(0);
  await expect(page.locator("#menu-run-folder .ff-icon")).toBeVisible();
  await expect(page.locator("#menu-run-folder")).toHaveCSS("color", await themeColor(page, "--danger"));
  await page.locator("#menu-run-folder").click();
  await expect(page.locator("#file-menu")).toBeHidden();
  await expect(page.locator("#result-meta")).toHaveText("Nothing to run");
  await expect(page.locator(".node-running")).toHaveCount(0);
});

test("creates a file inside a folder from the context menu", async ({ page }) => {
  const dialogs = handleDialogs(page, async (dialog) => {
    expect(dialog.defaultValue()).toBe("notes.dql");
    await dialog.accept("made.dql");
  });
  await node(page, "queries").click({ button: "right" });
  await page.locator("#menu-new-file").click();
  await expect(node(page, "queries/made.dql")).toBeVisible();
  await expect(page.locator("#file-name")).toHaveText("made.dql");
  await dialogs.stop();
});

test("duplicates a file and a folder", async ({ page }) => {
  await node(page, "queries/edit.dql").click({ button: "right" });
  await page.locator("#menu-duplicate").click();
  await expect(node(page, "queries/edit copy.dql")).toBeVisible();
  await expect(page.locator("#file-name")).toHaveText("edit copy.dql");

  await node(page, "empty").click({ button: "right" });
  await page.locator("#menu-duplicate").click();
  await expect(node(page, "empty copy")).toBeVisible();
});

test("renames from a double-click and cancels with Escape", async ({ page }) => {
  await node(page, "queries/read.dql").dblclick();
  const input = page.locator("#rename-input");
  await expect(input).toBeVisible();
  await expect(input).toHaveValue("read.dql");
  await input.fill("nope.dql");
  await input.press("Escape");
  await expect(node(page, "queries/read.dql")).toBeVisible();
  await expect(node(page, "queries/nope.dql")).toHaveCount(0);

  await node(page, "queries/edit.dql").dblclick();
  await page.locator("#rename-input").fill("renamed.dql");
  await page.locator("#rename-input").press("Enter");
  await expect(node(page, "queries/renamed.dql")).toBeVisible();
  await expect(page.locator("#file-name")).toHaveText("renamed.dql");

  await node(page, "empty").dblclick();
  await page.locator("#rename-input").fill("vacant");
  await page.locator("#rename-input").press("Enter");
  await expect(node(page, "vacant")).toBeVisible();
  await expect(node(page, "empty")).toHaveCount(0);
});

test("moves a file through the prompt and by dragging", async ({ page }) => {
  const dialogs = handleDialogs(page, async (dialog) => {
    expect(dialog.type()).toBe("prompt");
    expect(dialog.defaultValue()).toBe("queries/read.dql");
    await dialog.accept("empty/moved.dql");
  });
  await node(page, "queries/read.dql").click({ button: "right" });
  await page.locator("#menu-move").click();
  await expect(node(page, "empty/moved.dql")).toBeVisible();
  await expect(node(page, "queries/read.dql")).toHaveCount(0);
  await dialogs.stop();

  await node(page, "queries/edit.dql").dragTo(node(page, "empty"));
  await expect(node(page, "empty/edit.dql")).toBeVisible();
  await expect(page.locator("#file-name")).toHaveText("edit.dql");

  const tree = page.locator("#tree");
  const box = await tree.boundingBox();
  await node(page, "empty/moved.dql").dragTo(tree, {
    targetPosition: { x: 16, y: box.height - 2 },
  });
  await expect(node(page, "moved.dql")).toBeVisible();
  await expect(node(page, "empty/moved.dql")).toHaveCount(0);
});

test("deletes a file, deletes an empty folder, and refuses a folder that holds a non-dql file", async ({ page }) => {
  const dialogs = handleDialogs(page, async (dialog) => {
    await dialog.accept();
  });

  await node(page, "queries/read.dql").click({ button: "right" });
  await page.locator("#menu-delete").click();
  await expect(node(page, "queries/read.dql")).toHaveCount(0);

  await node(page, "empty").click({ button: "right" });
  await page.locator("#menu-duplicate").click();
  await expect(node(page, "empty copy")).toBeVisible();
  await node(page, "empty copy").click({ button: "right" });
  await page.locator("#menu-delete").click();
  await expect(node(page, "empty copy")).toHaveCount(0);

  await node(page, "empty").click({ button: "right" });
  await page.locator("#menu-delete").click();
  await expect.poll(() => dialogs.messages.map((item) => item.message).join("\n")).toContain(
    "folder contains .gitkeep, which is not a .dql file",
  );
  await expect(node(page, "empty")).toBeVisible();
  await dialogs.stop();
});

test("writes editor edits back to the temporary file", async ({ page, app }) => {
  await page.locator("#editor-content").click();
  await pressMod(page, "A");
  await page.keyboard.press("Backspace");
  await page.keyboard.insertText("-- saved from the test\nSELECT 1;\n");
  await expect.poll(() => fs.readFileSync(path.join(app.workspace, "queries/edit.dql"), "utf8"), {
    timeout: 5_000,
  }).toBe("-- saved from the test\nSELECT 1;\n");
  const source = fs.readFileSync(new URL("../fixtures/workspace/queries/edit.dql", import.meta.url), "utf8");
  expect(source).toBe("-- editable\nSELECT 1;\n");
});

test("says the server went away and retries the config call", async ({ page }) => {
  await page.route("**/api/**", (route) => route.abort());
  await node(page, "queries/read.dql").click();
  const pop = page.locator("#server-pop");
  await expect(pop).toBeVisible();
  await expect(page.locator("#server-pop-title")).toHaveText("The server went away");
  await expect(page.locator("#server-retry")).toBeVisible();

  await page.locator("#server-retry").click();
  await expect(page.locator("#server-pop-status")).toHaveText("Still unavailable.");
  await expect(pop).toBeVisible();

  const checks = [];
  await page.unroute("**/api/**");
  page.on("request", (request) => {
    if (new URL(request.url()).pathname === "/api/config") checks.push(request.method());
  });
  await page.locator("#server-retry").click();
  await expect(pop).toBeHidden();
  expect(checks).toEqual(["GET"]);
});
