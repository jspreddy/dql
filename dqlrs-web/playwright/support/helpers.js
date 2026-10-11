import { expect } from "@playwright/test";

// CodeMirror Mod is Command on macOS and Control elsewhere.
export const mod = process.platform === "darwin" ? "Meta" : "Control";

export async function pressMod(page, key) {
  await page.keyboard.press(`${mod}+${key}`);
}

export function node(page, filePath) {
  const id = "node-" + String(filePath).replaceAll("/", "__");
  return page.locator(`[data-testid="${id}"]`);
}

export async function openApp(page, app) {
  await page.goto(`${app.baseURL}/`);
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
}

export async function replaceEditor(page, text) {
  await page.locator("#editor-content").click();
  await pressMod(page, "A");
  await page.keyboard.press("Backspace");
  await page.keyboard.insertText(text);
}

export function handleDialogs(page, handler) {
  const messages = [];
  const listener = async (dialog) => {
    messages.push({ type: dialog.type(), message: dialog.message(), defaultValue: dialog.defaultValue() });
    await handler(dialog, messages);
  };
  page.on("dialog", listener);
  return {
    messages,
    async stop() {
      page.off("dialog", listener);
    },
  };
}

export async function lineOverlaps(page) {
  return page.evaluate(() => {
    const fill = document.querySelector(".cm-run-fill");
    if (!fill) return null;
    const box = fill.getBoundingClientRect();
    return [...document.querySelectorAll(".cm-content .cm-line")].map((line, index) => {
      const rect = line.getBoundingClientRect();
      const overlap = Math.max(0, Math.min(box.bottom, rect.bottom) - Math.max(box.top, rect.top));
      return {
        line: index + 1,
        text: line.textContent,
        height: rect.height,
        overlap,
      };
    });
  });
}

export function lineByText(overlaps, text) {
  return overlaps.find((line) => line.text.includes(text));
}
