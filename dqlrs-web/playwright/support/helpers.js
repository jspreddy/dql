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

export async function themeColor(page, name) {
  return page.evaluate((variable) => {
    const probe = document.createElement("span");
    probe.style.color = `var(${variable})`;
    document.body.append(probe);
    const color = getComputedStyle(probe).color;
    probe.remove();
    return color;
  }, name);
}

export function colorPattern(color) {
  return new RegExp(color.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
}

export async function openResultPane(page, pane) {
  // A finished run selects Result after it stores rows. Wait for that before
  // choosing another tab, or the store overwrites the click.
  await expect(page.locator("#run")).toBeEnabled();
  const tab = page.locator(`#result-tab-${pane}`);
  await tab.click();
  await expect(tab).toHaveAttribute("aria-selected", "true");
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
