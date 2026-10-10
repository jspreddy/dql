import fs from "node:fs";
import path from "node:path";
import { test, expect } from "../support/app.js";
import { handleDialogs, node, openApp } from "../support/helpers.js";

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

test("shows nested files, an empty folder, and opens a file", async ({ page }) => {
  await expect(node(page, "empty")).toBeVisible();
  await expect(node(page, "queries")).toBeVisible();
  await expect(node(page, "queries/edit.dql")).toBeVisible();
  await expect(node(page, "queries/read.dql")).toBeVisible();
  await expect(node(page, "queries/edit.dql")).toHaveClass(/selected/);

  await node(page, "queries").click();
  await expect(node(page, "queries/edit.dql")).toHaveCount(0);

  await node(page, "queries").click();
  await node(page, "queries/read.dql").click();
  await expect(page.locator("#file-name")).toHaveText("read.dql");
  await expect(node(page, "queries/read.dql")).toHaveClass(/selected/);
});

test("context menu on a file offers the file actions", async ({ page }) => {
  await node(page, "queries/edit.dql").click({ button: "right" });
  await expect(page.locator("#menu-new-file")).toBeVisible();
  await expect(page.locator("#menu-new-folder")).toBeVisible();
  await expect(page.locator("#menu-duplicate")).toBeVisible();
  await expect(page.locator("#menu-rename")).toBeVisible();
  await expect(page.locator("#menu-move")).toBeVisible();
  await expect(page.locator("#menu-delete")).toBeVisible();
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
  await expect(page.locator("#menu-duplicate")).toHaveCount(0);
  await page.locator("#menu-new-folder").click();
  await expect(node(page, "extra")).toBeVisible();
  await dialogs.stop();
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
  await page.keyboard.press("Control+A");
  await page.keyboard.press("Backspace");
  await page.keyboard.insertText("-- saved from the test\nSELECT 1;\n");
  await expect.poll(() => fs.readFileSync(path.join(app.workspace, "queries/edit.dql"), "utf8"), {
    timeout: 5_000,
  }).toBe("-- saved from the test\nSELECT 1;\n");
  const source = fs.readFileSync(new URL("../fixtures/workspace/queries/edit.dql", import.meta.url), "utf8");
  expect(source).toBe("-- editable\nSELECT 1;\n");
});
