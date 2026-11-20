import { test, expect } from "../support/app.js";
import { playwrightArtifact } from "../support/artifacts.js";
import { openApp, openResultPane, pressMod, replaceEditor } from "../support/helpers.js";

function uniqueName() {
  return `pw${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

// Same scenario as the engine test `two_queries_can_reuse_the_same_with_names`:
// two queries, then both in one script, share WITH names. Each query keeps its
// own rows. The names are not registered and do not publish a tables event.
test("two queries can reuse the same WITH names", async ({ page, app }, testInfo) => {
  await page.addInitScript(() => {
    window.__sseFrames = [];
    const original = window.fetch.bind(window);
    window.fetch = async (input, init) => {
      const response = await original(input, init);
      const url = typeof input === "string" ? input : input.url;
      if (!String(url).includes("/api/events") || !response.body) return response;
      const [stream, tap] = response.body.tee();
      const reader = tap.getReader();
      const decoder = new TextDecoder();
      (async () => {
        let buffer = "";
        while (true) {
          const { value, done } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });
          let split = buffer.indexOf("\n\n");
          while (split >= 0) {
            window.__sseFrames.push(buffer.slice(0, split));
            buffer = buffer.slice(split + 2);
            split = buffer.indexOf("\n\n");
          }
        }
      })();
      return new Response(stream, {
        status: response.status,
        statusText: response.statusText,
        headers: response.headers,
      });
    };
  });
  await openApp(page, app);

  const posts = uniqueName();
  const customers = uniqueName();
  const orders = `${posts}_orders`;
  const people = `${posts}_people`;
  app.trackTable(posts);
  app.trackTable(customers);

  async function runEditor(text) {
    await replaceEditor(page, text);
    await expect(page.locator("#editor-content")).toContainText(text.split("\n")[0]);
    await pressMod(page, "A");
    const finished = page.waitForResponse(
      (response) => response.url().includes("/api/run") && response.request().method() === "POST",
    );
    await page.locator("#run").click();
    const response = await finished;
    expect(response.ok(), await response.text()).toBe(true);
    await expect(page.locator("#run")).toBeEnabled();
  }

  function query(postId, customerId) {
    return [
      `WITH ${orders} AS (SCAN * FROM ${posts} WHERE id = '${postId}'),`,
      `     ${people} AS (SCAN * FROM ${customers} WHERE id = '${customerId}')`,
      `SELECT name FROM ${orders}`,
      `JOIN ${people} ON ${orders}.customer = ${people}.id;`,
    ].join("\n");
  }

  async function tableFrames() {
    return page.evaluate(() => (window.__sseFrames || []).filter((frame) => frame.includes("event: tables")));
  }

  async function listed(name) {
    return page.evaluate(async (table) => {
      const token = document.querySelector('meta[name="dqlrs-token"]')?.content || "";
      const response = await fetch(`/api/tables?name=${encodeURIComponent(table)}`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (!response.ok) throw new Error(`tables ${response.status}`);
      const body = await response.json();
      return body.tables || [];
    }, name);
  }

  await runEditor(
    [
      `DROP TABLE IF EXISTS ${posts};`,
      `DROP TABLE IF EXISTS ${customers};`,
      `CREATE TABLE ${posts} (id STRING HASH KEY, customer STRING);`,
      `CREATE TABLE ${customers} (id STRING HASH KEY, name STRING);`,
      `INSERT INTO ${posts} (id, customer) VALUES ('a', 'c1'), ('b', 'c2');`,
      `INSERT INTO ${customers} (id, name) VALUES ('c1', 'Ada'), ('c2', 'Grace');`,
    ].join("\n"),
  );
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect.poll(async () => (await tableFrames()).some((frame) => frame.includes(posts))).toBe(true);

  const before = await tableFrames();
  await runEditor(query("a", "c1"));
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator("#result-table")).toContainText("Ada");
  await expect(page.locator("#result-table")).not.toContainText("Grace");
  await expect(page.locator(".result-card").last().locator(".result-card-meta")).toHaveText("1 row");

  await runEditor(query("b", "c2"));
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await expect(page.locator("#result-table")).toContainText("Grace");
  await expect(page.locator("#result-table")).not.toContainText("Ada");
  await expect(page.locator(".result-card").last().locator(".result-card-meta")).toHaveText("1 row");
  await page.locator("#result-table").screenshot({
    path: playwrightArtifact(`with-same-names-${testInfo.project.name}.png`),
  });

  const during = (await tableFrames()).slice(before.length);
  expect(during.some((frame) => frame.includes(orders) || frame.includes(people))).toBe(false);
  expect(await listed(orders)).toEqual([]);
  expect(await listed(people)).toEqual([]);

  const cards = page.locator(".query-result");
  const beforeCards = await cards.count();
  await runEditor(`${query("a", "c1")}\n${query("b", "c2")}`);
  await expect(page.locator('[data-testid="result-note"].error')).toHaveCount(0);
  await openResultPane(page, "history");
  await expect(cards).toHaveCount(beforeCards + 2);
  const latest = cards.nth(beforeCards + 1);
  const previous = cards.nth(beforeCards);
  await expect(latest).toContainText("Grace");
  await expect(latest).not.toContainText("Ada");
  await expect(previous).toContainText("Ada");
  await expect(previous).not.toContainText("Grace");

  const afterScript = (await tableFrames()).slice(before.length);
  expect(afterScript.some((frame) => frame.includes(orders) || frame.includes(people))).toBe(false);
  expect(await listed(orders)).toEqual([]);
  expect(await listed(people)).toEqual([]);

  await page.locator("#mode-tables").click();
  await page.locator("#table-search").fill(orders);
  await expect(page.locator(`#table-${orders}`)).toHaveCount(0);
  await expect(page.locator("#table-list .table-name", { hasText: orders })).toHaveCount(0);
  await page.locator("#table-search").fill(people);
  await expect(page.locator(`#table-${people}`)).toHaveCount(0);
  await expect(page.locator("#table-list .table-name", { hasText: people })).toHaveCount(0);

  await page.locator("#mode-query").click();
  await runEditor(`SCAN * FROM ${orders};`);
  await expect(page.locator(".query-result").last().locator('[data-testid="result-note"].error')).toBeVisible();
  await expect(page.locator(".query-result").last()).toContainText(orders);
});
