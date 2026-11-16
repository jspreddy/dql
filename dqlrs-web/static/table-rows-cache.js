// Resident table-browser pages for one query run. Next/Previous reuse these
// instead of calling /api/rows again. A new Run (or a different table/DQL)
// drops the map so stale pages are not shown.

export const ROWS_CACHE_MAX_PAGES = 40;

export function createRowsCache({ maxPages = ROWS_CACHE_MAX_PAGES } = {}) {
  let table = "";
  let dql = "";
  let generation = 0;
  const pages = new Map();

  function sameQuery(nextTable, nextDql) {
    return table === nextTable && dql === (nextDql || "");
  }

  function evict() {
    while (pages.size > maxPages) {
      pages.delete(pages.keys().next().value);
    }
  }

  function touch(page, payload) {
    pages.delete(page);
    pages.set(page, payload);
    evict();
  }

  return {
    get generation() {
      return generation;
    },
    size() {
      return pages.size;
    },
    clear() {
      table = "";
      dql = "";
      pages.clear();
      generation += 1;
      return generation;
    },
    beginRun(nextTable, nextDql) {
      table = nextTable || "";
      dql = nextDql || "";
      pages.clear();
      generation += 1;
      return generation;
    },
    get(nextTable, nextDql, page) {
      if (!sameQuery(nextTable, nextDql)) return null;
      const payload = pages.get(page);
      if (!payload) return null;
      touch(page, payload);
      return payload;
    },
    set(nextTable, nextDql, page, payload) {
      if (!sameQuery(nextTable, nextDql)) this.beginRun(nextTable, nextDql);
      touch(page, payload);
      return payload;
    },
  };
}
