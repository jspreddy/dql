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

// Drag one element onto another with HTML5 drag events and a shared DataTransfer.
// Playwright's dragTo decides whether a drag started by waiting one task after the first
// mousemove for dragstart. When the page is busy (a file just reopened after a move, a lint
// running), dragstart lands later, Playwright drops its drag interception, and Chrome's own
// drag never ends. These events run the same tree handlers without that guess.
export async function dragAndDrop(page, source, target) {
  const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
  // Hold the source element itself: a successful drop repaints the tree, and the browser
  // still fires dragend on the original element even after it leaves the page.
  const dragged = await source.elementHandle();
  try {
    await dragged.dispatchEvent("dragstart", { dataTransfer });
    await target.dispatchEvent("dragenter", { dataTransfer });
    await target.dispatchEvent("dragover", { dataTransfer });
    await target.dispatchEvent("drop", { dataTransfer });
    await dragged.dispatchEvent("dragend", { dataTransfer });
  } finally {
    await dragged.dispose();
    await dataTransfer.dispose();
  }
}

// "Run file" and "Run folder" in the tree menu open the run dialog. Confirm, wait for the
// run to finish, and close the dialog.
export async function runFromDialog(page) {
  const dialog = page.locator("#run-pop");
  await expect(dialog).toBeVisible();
  await page.locator("#run-pop-start").click();
  await expect(page.locator("#run-pop-cancel")).toHaveText("Close");
  await page.locator("#run-pop-cancel").click();
  await expect(dialog).toBeHidden();
}

// The box of an editor line. CodeMirror replaces line elements when decorations change (lint
// marks and saved results land shortly after a file opens), and boundingBox() returns null
// for a line that is detached while it is measured. Wait for the line and measure again.
export async function lineBox(page, text) {
  const line = page.locator(".cm-line", { hasText: text });
  await expect(line).toBeVisible();
  let box = null;
  await expect.poll(async () => {
    box = await line.boundingBox();
    return box !== null;
  }).toBe(true);
  return box;
}

export async function reveal(page, filePath) {
  const parts = String(filePath).split("/");
  for (let i = 0; i < parts.length - 1; i += 1) {
    const dir = parts.slice(0, i + 1).join("/");
    const child = parts.slice(0, i + 2).join("/");
    if (await node(page, child).count() === 0) {
      await node(page, dir).click();
      // A folder click opens it 220 ms later, after ruling out a double-click. Wait for it so
      // the caller does not race that pending click.
      await expect(node(page, child)).toBeVisible();
    }
  }
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
  await reveal(page, "queries/edit.dql");
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
