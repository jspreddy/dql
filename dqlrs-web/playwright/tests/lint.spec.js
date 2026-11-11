import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import fs from "node:fs";
import path from "node:path";
import { node, openApp, replaceEditor, themeColor } from "../support/helpers.js";

const BROKEN = "SELEKT * FROM shelf;";
const UNKNOWN = "SELECT * FROM lint_missing_shelf WHERE id = 'a';";
const CLEAN = "CREATE TABLE shelf (id STRING HASH KEY);\nSELECT * FROM shelf WHERE id = 'a';";

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

test("underlines a bad query, warns when the table is unknown, and clears a valid script", async ({ page }, testInfo) => {
  await replaceEditor(page, BROKEN);
  const status = page.locator("#lint-status");
  await expect(status).toBeVisible();
  await expect(status).toHaveText("1 error");
  await expect(status).not.toHaveClass(/is-warning/);
  await expect(status).toHaveAttribute("data-tip", /expected a DQL statement/);
  await expect(status).not.toHaveAttribute("title");
  const mark = page.locator(".cm-lint-error");
  await expect(mark).toHaveCount(1);
  await expect(mark).toHaveAttribute("data-tip", /expected a DQL statement/);
  await expect(mark).not.toHaveAttribute("title");
  await expect(page.locator("[data-testid='lint-gutter-error']")).toHaveCount(1);
  await expect(status).toHaveCSS("color", await themeColor(page, "--danger"));
  await expect(status).toHaveCSS("background-color", await themeColor(page, "--danger-wash"));
  await expect(mark).toHaveCSS("text-decoration-color", await themeColor(page, "--danger-bright"));
  await expect(page.locator("[data-testid='lint-gutter-error']")).toHaveCSS(
    "color",
    await themeColor(page, "--danger-bright"),
  );

  const line = page.locator(".cm-line", { hasText: "SELEKT" });
  await line.hover();
  const tip = page.locator("#lint-tip");
  await expect(tip).toBeVisible();
  await expect(tip).toHaveText(/expected a DQL statement/);
  await page.screenshot({ path: playwrightArtifact(`lint-hover-${testInfo.project.name}.png`) });
  await page.mouse.move(0, 0);
  await expect(tip).toBeHidden();
  await expect(page.locator("[data-testid='lint-gutter-error']")).toHaveAttribute("data-tip", /expected a DQL statement/);
  await expect(page.locator("[data-testid='lint-gutter-error']")).not.toHaveAttribute("title");
  await page.locator("#collapse-tree").hover();
  await expect(tip).toHaveText("Collapse all folders");
  await expect(page.locator("#collapse-tree")).not.toHaveAttribute("title");
  const nativeTitles = await page.locator("[title]").evaluateAll((els) => els
    .filter((el) => el.tagName !== "IFRAME")
    .map((el) => `${el.tagName}.${el.id || el.className}`));
  expect(nativeTitles).toEqual([]);

  await status.click();
  await expect(page.locator("#result-tab-lint")).toHaveAttribute("aria-selected", "true");
  await expect(page.locator("#result-tab-result")).toHaveAttribute("aria-selected", "false");
  const row = page.locator("[data-testid='lint-row']");
  await expect(row).toHaveCount(1);
  await expect(row).toContainText("Error");
  await expect(row).toContainText("Line 1");
  await expect(row).toContainText(/expected a DQL statement/);
  await expect(page.locator("#result-count")).toHaveText("1 issue");
  await expect(page.locator(".query-result")).toHaveCount(0);
  await page.locator("#results").screenshot({
    path: playwrightArtifact(`lint-tab-${testInfo.project.name}.png`),
  });

  await row.click();
  await expect(page.locator("#editor-content")).toBeFocused();
  const cursorLine = await page.evaluate(() => {
    const cursor = document.querySelector(".cm-cursor");
    if (!cursor) return "";
    const box = cursor.getBoundingClientRect();
    const line = [...document.querySelectorAll(".cm-content .cm-line")].find((el) => {
      const rect = el.getBoundingClientRect();
      return box.top < rect.bottom && box.bottom > rect.top;
    });
    return line ? line.textContent : "";
  });
  expect(cursorLine).toContain("SELEKT");

  await replaceEditor(page, UNKNOWN);
  await expect(status).toBeVisible();
  await expect(status).toHaveText("1 warning");
  await expect(status).toHaveClass(/is-warning/);
  await expect(status).toHaveAttribute("data-tip", /no cached definition/);
  await expect(status).not.toHaveAttribute("title");
  await expect(page.locator(".cm-lint-error")).toHaveCount(0);
  const warning = page.locator(".cm-lint-warning");
  await expect(warning).toHaveCount(1);
  await expect(page.locator("[data-testid='lint-gutter-warning']")).toHaveCount(1);
  await expect(status).toHaveCSS("color", await themeColor(page, "--amber-ink"));
  await expect(status).toHaveCSS("background-color", await themeColor(page, "--amber-bg"));
  await expect(warning).toHaveCSS("text-decoration-color", await themeColor(page, "--amber"));
  await page.locator(".cm-line", { hasText: "lint_missing_shelf" }).hover();
  await expect(tip).toBeVisible();
  await expect(tip).toHaveText(/no cached definition/);
  await page.mouse.move(0, 0);
  await expect(tip).toBeHidden();
  await status.click();
  await expect(page.locator("#result-tab-lint")).toHaveAttribute("aria-selected", "true");
  await expect(row).toHaveCount(1);
  await expect(row).toContainText("Warning");
  await expect(row).toContainText(/no cached definition/);
  await expect(page.locator("#result-count")).toHaveText("1 issue");

  await replaceEditor(page, CLEAN);
  await expect(status).toBeHidden();
  await expect(page.locator(".cm-lint-error")).toHaveCount(0);
  await expect(page.locator(".cm-lint-warning")).toHaveCount(0);
  await expect(page.locator("[data-testid='lint-gutter-error']")).toHaveCount(0);
  await expect(page.locator("[data-testid='lint-gutter-warning']")).toHaveCount(0);
});

test("does not underline the comments above a statement", async ({ page }, testInfo) => {
  await replaceEditor(page, ["-- notes about the shelf", "-- second note line", UNKNOWN].join("\n"));
  const status = page.locator("#lint-status");
  await expect(status).toHaveText("1 warning");
  const marks = page.locator(".cm-lint-warning");
  await expect(marks.first()).toBeVisible();
  const underlined = (await marks.allTextContents()).join("");
  expect(underlined).toBe(UNKNOWN);
  const gutter = page.locator("[data-testid='lint-gutter-warning']");
  await expect(gutter).toHaveCount(1);
  const statementLine = await page.locator(".cm-line", { hasText: "lint_missing_shelf" }).boundingBox();
  const mark = await gutter.boundingBox();
  expect(Math.abs(mark.y + mark.height / 2 - (statementLine.y + statementLine.height / 2))).toBeLessThan(statementLine.height / 2);
  await page.screenshot({ path: playwrightArtifact(`lint-skips-comments-${testInfo.project.name}.png`) });
});

test("lints on open, after save, and when a fetch replaces the file", async ({ page, app }, testInfo) => {
  const broken = "SELEKT * FROM shelf;\n";
  const lints = [];
  const lintAfterSave = [];
  let saved = false;
  page.on("request", (request) => {
    const url = new URL(request.url());
    if (request.method() !== "POST" || url.pathname !== "/api/lint") return;
    lints.push(request.postData() || "");
    if (saved) lintAfterSave.push(1);
  });
  page.on("response", (response) => {
    const url = new URL(response.url());
    if (response.request().method() === "PUT" && url.pathname === "/api/file" && response.ok()) {
      saved = true;
    }
  });

  await replaceEditor(page, broken);
  const status = page.locator("#lint-status");
  await expect(status).toHaveText("1 error");
  await expect.poll(() => lintAfterSave.length).toBeGreaterThan(0);
  await page.screenshot({ path: playwrightArtifact(`lint-on-save-${testInfo.project.name}.png`) });

  const opened = path.join(app.workspace, "queries/lint-open.dql");
  fs.writeFileSync(opened, broken);
  await expect(node(page, "queries/lint-open.dql")).toBeVisible();
  const beforeOpen = lints.length;
  await node(page, "queries/lint-open.dql").click();
  await expect(page.locator("#file-name")).toHaveText("lint-open.dql");
  await expect.poll(() => lints.length).toBeGreaterThan(beforeOpen);
  await expect(status).toHaveText("1 error");
  await page.screenshot({ path: playwrightArtifact(`lint-on-open-${testInfo.project.name}.png`) });

  fs.writeFileSync(opened, CLEAN);
  await expect(page.locator("#editor-content")).toContainText("SELECT * FROM shelf");
  await expect(status).toBeHidden();
  await expect(page.locator(".cm-lint-error")).toHaveCount(0);
  await page.screenshot({ path: playwrightArtifact(`lint-on-fetch-${testInfo.project.name}.png`) });
});

test("a lint throw does not block editing recent.dql", async ({ page }, testInfo) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await replaceEditor(page, "SCAN * FROM nb_posts;\n");
  await replaceEditor(page, "SCAN * FROM");
  await expect(page.locator("#lint-status")).toBeVisible();
  await expect(page.locator(".cm-lint-error, .cm-lint-warning")).not.toHaveCount(0);
  await page.locator("#editor-content").click();
  await page.keyboard.press("End");
  await page.keyboard.type(" nb_posts");
  const edited = await page.evaluate(() => ({
    text: document.querySelector("#editor-content")?.innerText || "",
    marks: document.querySelectorAll(".cm-lint-error, .cm-lint-warning").length,
  }));
  expect(edited.text).toContain("SCAN * FROM nb_posts");
  expect(edited.marks).toBeGreaterThan(0);
  expect(errors).toEqual([]);
  await page.screenshot({ path: playwrightArtifact(`lint-edit-recent-${testInfo.project.name}.png`) });
});

test("a thrown lint update is logged and the edit still applies", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { EditorState } = await import("/vendor/codemirror-state.js");
    const { lintExtension, setLintIssues } = await import("/dql-mode.js");
    const logs = [];
    const original = console.error;
    console.error = (...args) => {
      logs.push(args.map((arg) => (arg instanceof Error ? arg.message : String(arg))).join(" "));
      original.apply(console, args);
    };
    const issue = {
      get offset() {
        throw new RangeError("Position 30 is out of range for changeset of length 29");
      },
      end: 4,
      severity: "error",
      message: "boom",
    };
    try {
      let state = EditorState.create({ doc: "SCAN * FROM", extensions: lintExtension });
      state = state.update({ effects: setLintIssues.of([issue]) }).state;
      state = state.update({ changes: { from: state.doc.length, insert: " x" } }).state;
      return { doc: state.doc.toString(), logs };
    } finally {
      console.error = original;
    }
  });
  expect(result.doc).toBe("SCAN * FROM x");
  expect(result.logs.join("\n")).toContain("Position 30 is out of range");
});
