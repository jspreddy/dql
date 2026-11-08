import fs from "node:fs";
import path from "node:path";
import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { handleDialogs, node, openApp, pressMod, replaceEditor, reveal, themeColor } from "../support/helpers.js";

test.beforeEach(async ({ page, app }) => {
  await openApp(page, app);
});

test("renders markdown as formatted text and html as a live page", async ({ page, app }, testInfo) => {
  fs.mkdirSync(path.join(app.workspace, "docs"));
  fs.writeFileSync(
    path.join(app.workspace, "docs/README.md"),
    [
      "# Harbor notes",
      "",
      "A paragraph with `code` and [the diagram](page.html).",
      "",
      "- pine",
      "- court",
      "",
      "| Unit | City |",
      "| --- | --- |",
      "| 2A | Seattle |",
      "",
      "```sql",
      "SELECT 1;",
      "```",
      "",
      "<script>alert(1)</script>",
      "",
    ].join("\n"),
  );
  fs.writeFileSync(
    path.join(app.workspace, "docs/page.html"),
    [
      "<!DOCTYPE html><html><body>",
      "<h1 id=\"live\">waiting</h1>",
      "<script>document.querySelector('#live').textContent = 'ran live';</script>",
      "</body></html>",
    ].join(""),
  );
  await page.reload();
  await showTypes(page, ["md", "html"]);
  await reveal(page, "docs/README.md");
  await expect(node(page, "docs").locator(".node-type use")).toHaveAttribute("href", "/vendor/icons/folder.svg#icon");
  await expect(node(page, "docs/README.md").locator(".node-type use")).toHaveAttribute("href", "/vendor/icons/description.svg#icon");
  await expect(node(page, "docs/page.html").locator(".node-type use")).toHaveAttribute("href", "/vendor/icons/html.svg#icon");
  await expect(node(page, "docs/README.md").locator(".node-ext, .file-kind")).toHaveCount(0);
  await node(page, "docs/README.md").click();
  await expect(page.locator("#doc-view")).toBeVisible();
  await expect(page.locator("#doc-meta")).toHaveText("Formatted");
  await expect(page.locator(".editor")).toBeHidden();
  const article = page.locator(".doc-markdown");
  await expect(article.locator("h1")).toHaveText("Harbor notes");
  await expect(article.locator("p code")).toHaveText("code");
  await expect(article.locator("li").first()).toHaveText("pine");
  await expect(article.locator("td").nth(1)).toHaveText("Seattle");
  await expect(article.locator("pre code")).toHaveText("SELECT 1;");
  await expect(article.locator("script")).toHaveCount(0);
  await expect(article).toContainText("<script>alert(1)</script>");
  await expect(page.locator("#run")).toBeHidden();
  await node(page, "docs/README.md").click({ button: "right" });
  await expect(page.locator("#menu-run-file")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await page.screenshot({ path: playwrightArtifact(`markdown-view-${testInfo.project.name}.png`) });

  await article.locator("a").click();
  const frame = page.frameLocator(".doc-frame");
  await expect(page.locator("#doc-meta")).toHaveText("Rendered page");
  await expect(frame.locator("#live")).toHaveText("ran live");
  await expect(page.locator(".editor")).toBeHidden();
  await expect(page.locator(".doc-frame")).toHaveAttribute("sandbox", "allow-scripts allow-popups");
  await page.screenshot({ path: playwrightArtifact(`html-view-${testInfo.project.name}.png`) });

  await reveal(page, "queries/read.dql");
  await node(page, "queries/read.dql").click();
  await expect(page.locator("#doc-view")).toBeHidden();
  await expect(page.locator(".editor")).toBeVisible();
  await expect(page.locator("#file-name")).toHaveText("read.dql");
});

test("renders a mermaid diagram in a markdown file", async ({ page, app }, testInfo) => {
  const requested = [];
  page.on("request", (request) => requested.push(request.url()));
  fs.mkdirSync(path.join(app.workspace, "docs"));
  fs.writeFileSync(
    path.join(app.workspace, "docs/diagram.md"),
    [
      "# Shop flow",
      "",
      "A paragraph beside the diagram.",
      "",
      "```mermaid",
      "flowchart LR",
      "  reader[read] --> parser[parse]",
      "  parser --> engine[run]",
      "  aside[note]",
      "```",
      "",
      "```sql",
      "SELECT 1;",
      "```",
      "",
    ].join("\n"),
  );
  await page.reload();
  await showTypes(page, ["md"]);
  await reveal(page, "docs/diagram.md");
  await node(page, "docs/diagram.md").click();
  const diagram = page.locator(".doc-markdown pre.mermaid svg");
  await expect(diagram).toBeVisible();
  expect(requested.some((url) => url.includes("/vendor/mermaid.js"))).toBe(true);
  expect(requested.some((url) => url.includes("jsdelivr.net"))).toBe(false);
  const parser = diagram.locator("g.node", { hasText: "parse" });
  const note = diagram.locator("g.node", { hasText: "note" });
  const link = diagram.locator("path.flowchart-link").first();
  await expect(parser).toBeVisible();
  await expect(page.locator(".doc-markdown pre code")).toHaveText("SELECT 1;");
  await parser.hover();
  await expect(diagram).toHaveClass(/chain/);
  await expect(parser).toHaveClass(/hot/);
  await expect(diagram.locator("g.node", { hasText: "read" })).toHaveClass(/hot/);
  await expect(diagram.locator("g.node", { hasText: "run" })).toHaveClass(/hot/);
  await expect(note).not.toHaveClass(/hot/);
  await expect(link).toHaveClass(/hot/);
  await parser.click();
  await expect(parser).toHaveClass(/pinned/);
  await page.locator(".doc-markdown h1").hover();
  await expect(diagram).toHaveClass(/chain/);
  await expect(parser).toHaveClass(/pinned/);
  await parser.click();
  await expect(parser).not.toHaveClass(/pinned/);
  const paper = await themeColor(page, "--paper");
  const text = await themeColor(page, "--text");
  const line = await themeColor(page, "--line-control");
  const lineSoft = await themeColor(page, "--line");
  const shadow = await themeColor(page, "--shadow-pop");
  const nodeRect = diagram.locator("g.node rect").first();
  await expect(nodeRect).toHaveCSS("fill", paper);
  await expect(diagram.locator(".nodeLabel").first()).toHaveCSS("color", text);
  const stops = await diagram.locator("linearGradient stop").evaluateAll((els) => els.map((el) => getComputedStyle(el).stopColor));
  expect(stops).toEqual([line, lineSoft]);
  expect(await nodeRect.evaluate((el) => getComputedStyle(el).filter)).toContain(shadow);
  await expect(page.locator(".mermaid-expand")).toHaveCount(1);
  await page.screenshot({ path: playwrightArtifact(`mermaid-view-${testInfo.project.name}.png`) });

  await page.locator(".mermaid-expand").click();
  const screen = page.locator("#mermaid-screen");
  await expect(screen).toBeVisible();
  const canvas = page.locator("#mermaid-screen-canvas");
  await expect(canvas.locator("g.node rect").first()).toHaveCSS("fill", paper);
  expect(await canvas.locator("g.node rect").first().evaluate((el) => getComputedStyle(el).filter)).toContain(shadow);
  const fitted = await canvas.evaluate((el) => el.getBoundingClientRect().width);
  const stage = page.locator("#mermaid-screen-stage");
  await stage.hover();
  for (let step = 0; step < 12; step += 1) await page.mouse.wheel(0, -240);
  const zoomedIn = await canvas.evaluate((el) => el.getBoundingClientRect().width);
  expect(zoomedIn).toBeGreaterThan(fitted * 2);
  expect(Number.parseInt(await page.locator("#mermaid-zoom-label").textContent(), 10)).toBeGreaterThan(200);
  for (let step = 0; step < 24; step += 1) await page.mouse.wheel(0, 240);
  expect(await canvas.evaluate((el) => el.getBoundingClientRect().width)).toBeLessThan(fitted * 0.8);
  expect(Number.parseInt(await page.locator("#mermaid-zoom-label").textContent(), 10)).toBeLessThan(80);
  await page.mouse.wheel(0, 240);
  const spot = await canvas.boundingBox();
  await page.mouse.move(spot.x + 30, spot.y + 24);
  await page.mouse.down();
  await page.mouse.move(spot.x + 110, spot.y + 80);
  await page.mouse.up();
  const moved = await canvas.boundingBox();
  expect(moved.x).toBeGreaterThan(spot.x + 50);
  expect(moved.y).toBeGreaterThan(spot.y + 30);
  await page.locator("#mermaid-zoom-fit").click();
  await expect(page.locator("#mermaid-zoom-label")).toHaveText("100%");
  await page.keyboard.press("Escape");
  await expect(screen).toBeHidden();
});

test("resolves relative links from the file's folder", async ({ page, app }) => {
  fs.mkdirSync(path.join(app.workspace, "docs/nested"), { recursive: true });
  fs.writeFileSync(
    path.join(app.workspace, "docs/README.md"),
    ["# Docs", "", "See [the page](nested/page.html) and [the outside note](../outside.md).", ""].join("\n"),
  );
  fs.writeFileSync(path.join(app.workspace, "outside.md"), "# Outside\n");
  fs.writeFileSync(path.join(app.workspace, "docs/nested/note.md"), "# Nested note\n");
  fs.writeFileSync(
    path.join(app.workspace, "docs/nested/page.html"),
    [
      "<!DOCTYPE html><html><body>",
      "<h1 id=\"live\">waiting</h1>",
      "<a id=\"up\" href=\"../README.md\">Up</a>",
      "<a id=\"side\" href=\"note.md\">Note</a>",
      "<a id=\"out\" href=\"https://example.com/away\">Away</a>",
      "<script>document.querySelector('#live').textContent = 'ran';</script>",
      "</body></html>",
    ].join(""),
  );
  await page.reload();
  await showTypes(page, ["md", "html"]);
  await reveal(page, "docs/README.md");

  await node(page, "docs/README.md").click();
  await page.locator(".doc-markdown a", { hasText: "the outside note" }).click();
  await expect(page.locator("#doc-title")).toHaveText("outside.md");
  await expect(page.locator(".doc-markdown h1")).toHaveText("Outside");

  await reveal(page, "docs/README.md");
  await node(page, "docs/README.md").click();
  await page.locator(".doc-markdown a", { hasText: "the page" }).click();
  const frame = page.frameLocator(".doc-frame");
  await expect(frame.locator("#live")).toHaveText("ran");
  await expect(frame.locator("#out")).toHaveAttribute("href", "https://example.com/away");
  await frame.locator("#side").click();
  await expect(page.locator("#doc-title")).toHaveText("note.md");
  await expect(page.locator(".doc-markdown h1")).toHaveText("Nested note");

  await reveal(page, "docs/nested/page.html");
  await node(page, "docs/nested/page.html").click();
  await frame.locator("#up").click();
  await expect(page.locator("#doc-title")).toHaveText("README.md");
  await expect(page.locator(".doc-markdown h1")).toHaveText("Docs");
  await expect(node(page, "docs/README.md")).toHaveClass(/selected/);
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

test("keeps the title icons on one row and the type pills below", async ({ page }, testInfo) => {
  const title = await page.locator("#side-title").boundingBox();
  const collapse = await page.locator("#collapse-tree").boundingBox();
  const expand = await page.locator("#expand-tree").boundingBox();
  const add = await page.locator("#new-file").boundingBox();
  const filters = await page.locator("#file-filters").boundingBox();
  const tree = await page.locator("#tree").boundingBox();
  const centers = [title, collapse, expand, add].map((box) => box.y + box.height / 2);
  expect(Math.max(...centers) - Math.min(...centers)).toBeLessThan(2);
  expect(expand.x).toBeGreaterThan(collapse.x + collapse.width - 1);
  expect(add.x).toBeGreaterThan(expand.x + expand.width - 1);
  expect(filters.y).toBeGreaterThan(title.y + title.height);
  expect(tree.y - (filters.y + filters.height)).toBeGreaterThanOrEqual(6);
  await page.screenshot({ path: playwrightArtifact(`tree-title-${testInfo.project.name}.png`) });
});

test("starts with folders collapsed and folds them from the title bar", async ({ page }, testInfo) => {
  await page.reload();
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  await expect(node(page, "queries")).toBeVisible();
  await expect(node(page, "empty")).toBeVisible();
  await expect(node(page, "queries/edit.dql")).toHaveCount(0);
  await page.screenshot({ path: playwrightArtifact(`tree-collapsed-${testInfo.project.name}.png`) });
  await expect(page.locator("#collapse-tree")).toBeVisible();
  await expect(page.locator("#expand-tree")).toBeVisible();

  await page.locator("#expand-tree").click();
  await expect(node(page, "queries/edit.dql")).toBeVisible();
  await expect(node(page, "queries/read.dql")).toBeVisible();
  await expect(node(page, "queries/functions.dql")).toBeVisible();
  await page.screenshot({ path: playwrightArtifact(`tree-expanded-${testInfo.project.name}.png`) });

  await page.locator("#collapse-tree").click();
  await expect(node(page, "queries")).toBeVisible();
  await expect(node(page, "queries/edit.dql")).toHaveCount(0);

  await page.locator("#mode-tables").click();
  await expect(page.locator("#tree-fold")).toBeHidden();
  await page.locator("#mode-query").click();
  await expect(page.locator("#collapse-tree")).toBeVisible();
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

test("does not refetch the tree while opening files or folders", async ({ page, app }) => {
  let treeFetches = 0;
  page.on("request", (request) => {
    const url = new URL(request.url());
    if (url.pathname === "/api/tree") treeFetches += 1;
  });
  await page.reload();
  await expect(page.locator("#file-name")).toHaveText("edit.dql");
  const afterBoot = treeFetches;
  expect(afterBoot).toBeGreaterThan(0);

  await page.locator("#expand-tree").click();
  await expect(node(page, "queries/edit.dql")).toBeVisible();
  await node(page, "queries").click();
  await expect(node(page, "queries/edit.dql")).toHaveCount(0);
  await node(page, "queries").click();
  await node(page, "queries/read.dql").click();
  await expect(page.locator("#file-name")).toHaveText("read.dql");
  await node(page, "queries/functions.dql").click();
  await expect(page.locator("#file-name")).toHaveText("functions.dql");
  await node(page, "empty").click();
  expect(treeFetches).toBe(afterBoot);

  await showTypes(page, ["md"]);
  fs.writeFileSync(path.join(app.workspace, "arrived.md"), "# Arrived\n");
  await expect(node(page, "arrived.md")).toBeVisible();
  expect(treeFetches).toBeGreaterThan(afterBoot);
});

async function showTypes(page, types) {
  for (const type of types) {
    const pill = page.locator(`.file-filter[data-type="${type}"]`);
    if ((await pill.getAttribute("aria-pressed")) !== "true") await pill.click();
  }
}

test("filters files with toggleable type pills and remembers them", async ({ page, app }, testInfo) => {
  fs.writeFileSync(path.join(app.workspace, "users.csv"), "id,name\na,ada\n");
  fs.writeFileSync(path.join(app.workspace, "cities.json"), "[{\"id\":\"a\"}]\n");
  fs.writeFileSync(path.join(app.workspace, "notes.md"), "# Notes\n");
  fs.writeFileSync(path.join(app.workspace, "page.html"), "<h1>Page</h1>\n");
  await page.reload();
  const pills = page.locator(".file-filter");
  await expect(pills).toHaveCount(5);
  await expect(pills).toHaveText(["CSV", "JSON", "DQL", "Markdown", "HTML"]);
  await expect(page.locator('.file-filter[data-type="dql"]')).toHaveAttribute("aria-pressed", "true");
  for (const type of ["csv", "json", "md", "html"]) {
    await expect(page.locator(`.file-filter[data-type="${type}"]`)).toHaveAttribute("aria-pressed", "false");
  }
  await reveal(page, "queries/edit.dql");
  await expect(node(page, "queries/edit.dql")).toBeVisible();
  await expect(node(page, "users.csv")).toHaveCount(0);
  await expect(node(page, "cities.json")).toHaveCount(0);
  await expect(node(page, "notes.md")).toHaveCount(0);
  await expect(node(page, "page.html")).toHaveCount(0);

  await showTypes(page, ["csv", "json", "md", "html"]);
  await expect(node(page, "users.csv")).toBeVisible();
  await expect(node(page, "cities.json")).toBeVisible();
  await expect(node(page, "notes.md")).toBeVisible();
  await expect(node(page, "page.html")).toBeVisible();
  await expect(node(page, "queries/edit.dql")).toBeVisible();
  const colors = await page.evaluate(() => {
    const read = (selector) => {
      const style = getComputedStyle(document.querySelector(selector));
      return [style.backgroundColor, style.color];
    };
    const ink = (selector) => getComputedStyle(document.querySelector(selector)).color;
    return {
      csvPill: read('.file-filter[data-type="csv"]'),
      jsonPill: read('.file-filter[data-type="json"]'),
      mdPill: read('.file-filter[data-type="md"]'),
      htmlPill: read('.file-filter[data-type="html"]'),
      csvName: ink('[data-testid="node-users.csv"] .node-name'),
      jsonName: ink('[data-testid="node-cities.json"] .node-name'),
      mdName: ink('[data-testid="node-notes.md"] .node-name'),
      htmlName: ink('[data-testid="node-page.html"] .node-name'),
      dqlName: ink('[data-testid="node-queries__edit.dql"] .node-name'),
      csvIcon: ink('[data-testid="node-users.csv"] .node-type'),
      jsonIcon: ink('[data-testid="node-cities.json"] .node-type'),
      mdIcon: ink('[data-testid="node-notes.md"] .node-type'),
      htmlIcon: ink('[data-testid="node-page.html"] .node-type'),
      dqlIcon: ink('[data-testid="node-queries__edit.dql"] .node-type'),
      folderIcon: ink('[data-testid="node-queries"] .node-type'),
    };
  });
  expect(colors.csvPill).toEqual(colors.jsonPill);
  expect(colors.mdPill).toEqual(colors.htmlPill);
  const fileName = await themeColor(page, "--text-secondary");
  expect(colors.csvName).toBe(fileName);
  expect(colors.jsonName).toBe(fileName);
  expect(colors.mdName).toBe(fileName);
  expect(colors.htmlName).toBe(fileName);
  expect(colors.dqlName).toBe(fileName);
  expect(colors.csvIcon).toBe(colors.jsonIcon);
  expect(colors.csvIcon).toBe(await themeColor(page, "--amber-ink"));
  expect(colors.mdIcon).toBe(colors.htmlIcon);
  expect(colors.mdIcon).toBe(await themeColor(page, "--blue-ink"));
  expect(colors.dqlIcon).toBe(await themeColor(page, "--teal-ink"));
  expect(colors.folderIcon).toBe(await themeColor(page, "--muted"));
  await expect(node(page, "users.csv").locator(".node-type use")).toHaveAttribute("href", "/vendor/icons/code.svg#icon");
  await expect(node(page, "cities.json").locator(".node-type use")).toHaveAttribute("href", "/vendor/icons/code.svg#icon");
  await expect(node(page, "notes.md").locator(".node-type use")).toHaveAttribute("href", "/vendor/icons/description.svg#icon");
  await expect(node(page, "page.html").locator(".node-type use")).toHaveAttribute("href", "/vendor/icons/html.svg#icon");
  await expect(node(page, "queries/edit.dql").locator(".node-type use")).toHaveAttribute("href", "/vendor/icons/terminal.svg#icon");
  await expect(node(page, "queries").locator(".node-type use")).toHaveAttribute("href", "/vendor/icons/folder.svg#icon");
  await expect(page.locator(".tree .file-kind, .tree .node-ext")).toHaveCount(0);
  await page.screenshot({ path: playwrightArtifact(`file-type-pills-${testInfo.project.name}.png`) });
  await page.locator('.file-filter[data-type="json"]').click();
  await page.locator('.file-filter[data-type="html"]').click();
  await expect(node(page, "cities.json")).toHaveCount(0);
  await expect(node(page, "page.html")).toHaveCount(0);

  await page.reload();
  await expect(page.locator('.file-filter[data-type="csv"]')).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator('.file-filter[data-type="md"]')).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator('.file-filter[data-type="dql"]')).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator('.file-filter[data-type="json"]')).toHaveAttribute("aria-pressed", "false");
  await expect(node(page, "users.csv")).toBeVisible();
  await expect(node(page, "notes.md")).toBeVisible();
  await expect(node(page, "cities.json")).toHaveCount(0);
  const stored = await page.evaluate(() => localStorage.getItem("dqlrs-web.file-types"));
  expect(JSON.parse(stored)).toEqual(["csv", "dql", "md"]);

  await page.locator('.file-filter[data-type="dql"]').click();
  await expect(node(page, "queries/edit.dql")).toHaveCount(0);
  await expect(node(page, "users.csv")).toBeVisible();
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

test("shows csv and json files as tables", async ({ page, app }, testInfo) => {
  fs.writeFileSync(path.join(app.workspace, "users.csv"), "id,name\na,ada\nb,grace\n");
  fs.writeFileSync(path.join(app.workspace, "cities.json"), "[{\"id\":\"a\",\"city\":\"nyc\"}]\n");
  fs.writeFileSync(path.join(app.workspace, "notes.txt"), "hidden\n");
  const many = ["id,n", ...Array.from({ length: 51 }, (_, index) => `r${index},${index}`)].join("\n") + "\n";
  fs.writeFileSync(path.join(app.workspace, "many.csv"), many);
  await page.reload();
  await showTypes(page, ["csv", "json"]);
  await expect(node(page, "users.csv")).toBeVisible();
  await expect(node(page, "cities.json")).toBeVisible();
  await expect(node(page, "notes.txt")).toHaveCount(0);
  await expect(node(page, "users.csv").locator(".node-type use")).toHaveAttribute("href", "/vendor/icons/code.svg#icon");
  await expect(node(page, "cities.json").locator(".node-type use")).toHaveAttribute("href", "/vendor/icons/code.svg#icon");

  await node(page, "users.csv").click();
  await expect(page.locator("#query-view")).toHaveClass(/is-data/);
  await expect(page.locator("#editor")).toBeHidden();
  await expect(page.locator("#results")).toBeHidden();
  await expect(page.locator("#file-rows")).toBeVisible();
  await expect(page.locator("#file-rows-title")).toHaveText("users.csv");
  await expect(page.locator("#file-rows-meta")).toHaveText("50 rows at a time");
  await expect(page.locator("#file-page-label")).toHaveText("Showing 1–2");
  await expect(page.locator("#file-page-prev")).toBeDisabled();
  await expect(page.locator("#file-page-next")).toBeDisabled();
  await expect(page.locator("#file-rows-table tbody tr")).toHaveCount(2);
  await expect(page.locator("#file-rows-table")).toContainText("ada");
  await expect(page.locator("#file-rows-table")).toContainText("grace");
  await expect(page.locator("#file-rows-table [data-key='hash']")).toBeVisible();
  const schema = page.locator("#file-rows-schema");
  await expect(schema).toBeVisible();
  await expect(schema).toHaveAttribute("aria-label", "Show result structure");
  const titleBox = await page.locator("#file-rows-title").boundingBox();
  const schemaBox = await schema.boundingBox();
  expect(schemaBox.x).toBeGreaterThan(titleBox.x + titleBox.width - 2);
  expect(Math.abs(schemaBox.y + schemaBox.height / 2 - (titleBox.y + titleBox.height / 2))).toBeLessThan(8);

  const duringStructure = [];
  const watchStructure = (request) => {
    const requestPath = new URL(request.url()).pathname;
    if (requestPath === "/api/file-rows" || requestPath === "/api/run" || requestPath === "/api/rows") {
      duringStructure.push(requestPath);
    }
  };
  page.on("request", watchStructure);
  await schema.click();
  const pop = page.locator("#schema-pop");
  await expect(pop).toBeVisible();
  await expect(page.locator("#schema-pop-kicker")).toHaveText("Result structure");
  await expect(page.locator("#schema-pop-info")).toBeVisible();
  await expect(page.locator("#schema-pop-info")).toHaveAttribute(
    "data-tip",
    "Based on every row in the file.",
  );
  await expect(page.locator("#schema-pop-title")).toHaveText("users.csv");
  await expect(page.locator("#schema-pop-summary")).toHaveText("2 columns · 2 rows");
  await expect(page.locator("#schema-pop-query")).toBeHidden();
  await expect(page.locator(".schema-pop-title-row .schema-key-chip")).toHaveCount(1);
  await expect(page.locator(".schema-pop-title-row .schema-key-chip")).toContainText("id");
  const idRow = page.locator('#schema-table tbody tr[data-column="id"]');
  await expect(idRow).toContainText("STRING");
  await expect(idRow).toContainText("Hash key");
  await expect(idRow).toContainText("2/2");
  await expect(idRow.locator(".schema-sample")).toHaveText("a");
  const nameRow = page.locator('#schema-table tbody tr[data-column="name"]');
  await expect(nameRow).toContainText("STRING");
  await expect(nameRow.locator(".schema-role-muted")).toHaveText("—");
  await expect(nameRow).toContainText("2/2");
  await expect(nameRow.locator(".schema-sample")).toHaveText("ada");
  await expect(page.locator("#schema-pop-foot")).toHaveText("Types come from the whole file. No extra query was run.");
  expect(duringStructure).toEqual([]);
  page.off("request", watchStructure);
  await page.screenshot({ path: playwrightArtifact(`file-structure-${testInfo.project.name}.png`) });
  await page.keyboard.press("Escape");
  await expect(pop).toBeHidden();

  await node(page, "users.csv").click({ button: "right" });
  await expect(page.locator("#menu-run-file")).toHaveCount(0);
  await page.locator("#file-rows-title").click();
  await page.screenshot({ path: playwrightArtifact(`view-csv-${testInfo.project.name}.png`) });

  const before = fs.readFileSync(path.join(app.workspace, "users.csv"), "utf8");
  const rejected = await page.evaluate(async (path) => {
    const saved = await fetch("/api/file", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path, text: "changed\n" }),
    });
    return saved.ok ? "" : await saved.text();
  }, "users.csv");
  expect(rejected).toContain("only .dql files can be changed");
  expect(fs.readFileSync(path.join(app.workspace, "users.csv"), "utf8")).toBe(before);

  await node(page, "cities.json").click();
  await expect(page.locator("#file-rows-title")).toHaveText("cities.json");
  await expect(page.locator("#editor")).toBeHidden();
  await expect(page.locator("#file-rows-table")).toContainText("nyc");
  await expect(page.locator("#file-page-label")).toHaveText("Showing 1–1");
  await page.locator("#file-rows-schema").click();
  await expect(page.locator("#schema-pop-title")).toHaveText("cities.json");
  await expect(page.locator("#schema-pop-kicker")).toHaveText("Result structure");
  await expect(page.locator('#schema-table tbody tr[data-column="city"]')).toContainText("STRING");
  await expect(page.locator('#schema-table tbody tr[data-column="city"] .schema-sample')).toHaveText("nyc");
  await page.keyboard.press("Escape");
  await page.screenshot({ path: playwrightArtifact(`view-json-${testInfo.project.name}.png`) });

  await node(page, "many.csv").click();
  await expect(page.locator("#file-rows-table tbody tr")).toHaveCount(50);
  await expect(page.locator("#file-page-label")).toHaveText("Showing 1–50");
  await expect(page.locator("#file-page-next")).toBeEnabled();
  await page.locator("#file-page-next").click();
  await expect(page.locator("#file-page-label")).toHaveText("Showing 51–51");
  await expect(page.locator("#file-rows-table tbody tr")).toHaveCount(1);
  await expect(page.locator("#file-rows-table")).toContainText("r50");
  await expect(page.locator("#file-page-next")).toBeDisabled();
  await expect(page.locator("#file-page-prev")).toBeEnabled();
  await page.locator("#file-rows-schema").click();
  await expect(page.locator("#schema-pop-summary")).toHaveText("2 columns · 51 rows");
  await expect(page.locator('#schema-table tbody tr[data-column="n"]')).toContainText("NUMBER");
  await expect(page.locator('#schema-table tbody tr[data-column="n"] .schema-sample')).toHaveText("0");
  await page.keyboard.press("Escape");

  await reveal(page, "queries/read.dql");
  await node(page, "queries/read.dql").click();
  await expect(page.locator("#file-name")).toHaveText("read.dql");
  await expect(page.locator("#file-rows")).toBeHidden();
  await expect(page.locator("#editor")).toBeVisible();
  await expect(page.locator("#editor-content")).toHaveAttribute("contenteditable", "true");
  await expect(page.locator("#run")).toBeEnabled();
  await expect(page.locator("#schema-pop")).toBeHidden();
  await expect(page.locator("#file-rows-schema")).toBeHidden();
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
  await reveal(page, "queries/read.dql");
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

test("deletes a file and an empty folder, and refuses a folder that has contents", async ({ page }) => {
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

  await node(page, "queries").click({ button: "right" });
  await page.locator("#menu-delete").click();
  await node(page, "empty").click({ button: "right" });
  await page.locator("#menu-delete").click();
  await expect.poll(() => dialogs.messages.filter((item) => item.message === "Folder is not empty, so unable to delete it from here.")).toHaveLength(2);
  await expect(node(page, "queries")).toBeVisible();
  await expect(node(page, "queries/edit.dql")).toBeVisible();
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
