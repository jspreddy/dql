// A bare page for component tests: the app's stylesheets (theme.css, app.css) and an empty #root, served on the app
// origin so `import("/<module>.js")` loads the same files the app ships. The theme follows the
// Playwright project (light or dark).

export const HARNESS_PATH = "/__component-harness__";

export async function openComponentHarness(page, app, testInfo) {
  const theme = testInfo.project.name === "dark" ? "dark" : "light";
  await page.route(`${app.baseURL}${HARNESS_PATH}`, (route) =>
    route.fulfill({
      contentType: "text/html; charset=utf-8",
      body: `<!doctype html>
<html lang="en" data-theme="${theme}">
<head><meta charset="utf-8"><link rel="stylesheet" href="/theme.css"><link rel="stylesheet" href="/app.css"><title>Component harness</title></head>
<body><div id="root" style="padding: 16px"></div></body>
</html>`,
    }),
  );
  await page.goto(`${app.baseURL}${HARNESS_PATH}`);
}
