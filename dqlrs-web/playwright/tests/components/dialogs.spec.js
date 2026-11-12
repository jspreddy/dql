import { test, expect } from "../../support/app.js";
import { playwrightArtifact } from "../../support/artifacts.js";
import { openComponentHarness } from "../../support/components.js";

// dialogs.js: the confirm and prompt dialogs.

test.beforeEach(async ({ page, app }, testInfo) => {
  await openComponentHarness(page, app, testInfo);
  await page.evaluate(async () => {
    const { createConfirmDialog, createPromptDialog } = await import("/dialogs.js");
    window.confirmDialog = createConfirmDialog();
    window.promptDialog = createPromptDialog();
  });
});

const answer = (page) => page.evaluate(() => window.answer);

test("confirm resolves true for OK and false for Cancel, Escape, and the backdrop", async ({ page }, testInfo) => {
  await page.evaluate(() => {
    window.answer = window.confirmDialog("Delete notes.dql?", { title: "Delete", path: "queries/notes.dql", okLabel: "Delete", danger: true });
  });
  const dialog = page.locator("#confirm-pop");
  await expect(dialog).toBeVisible();
  await expect(page.locator("#confirm-pop-title")).toHaveText("Delete");
  await expect(page.locator("#confirm-pop-path")).toHaveText("queries/notes.dql");
  await expect(page.locator("#confirm-pop-message")).toHaveText("Delete notes.dql?");
  await expect(page.locator("#confirm-ok")).toHaveText("Delete");
  await expect(page.locator("#confirm-ok")).toHaveClass(/danger/);
  await expect(page.locator("#confirm-cancel")).toBeFocused();
  await page.screenshot({ path: playwrightArtifact(`component-confirm-${testInfo.project.name}.png`) });
  await page.locator("#confirm-ok").click();
  await expect(dialog).toBeHidden();
  expect(await answer(page)).toBe(true);

  for (const dismiss of ["cancel", "escape", "backdrop"]) {
    await page.evaluate(() => {
      window.answer = window.confirmDialog("Again?");
    });
    await expect(dialog).toBeVisible();
    await expect(page.locator("#confirm-pop-path")).toBeHidden();
    await expect(page.locator("#confirm-ok")).not.toHaveClass(/danger/);
    if (dismiss === "cancel") await page.locator("#confirm-cancel").click();
    if (dismiss === "escape") await page.keyboard.press("Escape");
    if (dismiss === "backdrop") await page.mouse.click(5, 5);
    await expect(dialog).toBeHidden();
    expect(await answer(page), dismiss).toBe(false);
  }
});

test("asking again answers the open confirm false", async ({ page }) => {
  const answers = await page.evaluate(async () => {
    const first = window.confirmDialog("First?");
    const second = window.confirmDialog("Second?");
    document.querySelector("#confirm-ok").click();
    return [await first, await second];
  });
  expect(answers).toEqual([false, true]);
});

test("prompt selects the default, returns the entry, and null when cancelled", async ({ page }) => {
  await page.evaluate(() => {
    window.answer = window.promptDialog("New file name", "notes.dql", { okLabel: "Create" });
  });
  const dialog = page.locator("#prompt-pop");
  await expect(dialog).toBeVisible();
  await expect(page.locator("#prompt-pop-title")).toHaveText("New file name");
  await expect(page.locator("#prompt-ok")).toHaveText("Create");
  const input = page.locator("#prompt-pop-input");
  await expect(input).toBeFocused();
  await expect(input).toHaveValue("notes.dql");
  expect(await input.evaluate((el) => [el.selectionStart, el.selectionEnd])).toEqual([0, 9]);
  await page.keyboard.type("made.dql");
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
  expect(await answer(page)).toBe("made.dql");

  await page.evaluate(() => {
    window.answer = window.promptDialog("Rename", null);
  });
  await expect(input).toHaveValue("");
  await page.locator("#prompt-cancel").click();
  expect(await answer(page)).toBe(null);
});
